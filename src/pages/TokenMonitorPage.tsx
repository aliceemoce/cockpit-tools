import { useState, useEffect, useCallback, useRef, useMemo } from 'react';
import { readTextFile, writeTextFile } from '@tauri-apps/plugin-fs';
import { homeDir, join } from '@tauri-apps/api/path';
import { openPath } from '@tauri-apps/plugin-opener';
import {
    Zap,
    DollarSign,
    ArrowDownToLine,
    ArrowUpFromLine,
    FolderOpen,
    RefreshCw,
    TrendingUp,
    BarChart3,
    AlertCircle,
    FileQuestion,
    Radio,
    ChevronDown,
    Clock,
} from 'lucide-react';
import './TokenMonitorPage.css';

// ── Types ──

interface UsageRecord {
    timestamp: string;
    model: string;
    source?: string;
    input_tokens_est: number;
    output_tokens_est: number;
    total_tokens_est: number;
    input_cost_est: number;
    output_cost_est: number;
    total_cost_est: number;
    delta_bytes_in: number;
    delta_bytes_out: number;
}

interface ChartPoint {
    label: string;
    cost: number;
}

interface ModelSummary {
    model: string;
    inputTokens: number;
    outputTokens: number;
    cost: number;
    records: number;
}

type TimeRange = '10min' | '1h' | '1day' | 'overall';

// ── Model catalog (must match models.json) ──

const MODEL_CATALOG: Record<string, { name: string; shortcut: string }> = {
    'gemini-3.1-pro-high': { name: 'Gemini 3.1 Pro (High)', shortcut: '1' },
    'gemini-3.1-pro-low': { name: 'Gemini 3.1 Pro (Low)', shortcut: '2' },
    'gemini-3-flash': { name: 'Gemini 3 Flash', shortcut: '3' },
    'claude-sonnet-4.6-thinking': { name: 'Claude Sonnet 4.6 (Thinking)', shortcut: '4' },
    'claude-opus-4.6-thinking': { name: 'Claude Opus 4.6 (Thinking)', shortcut: '5' },
    'gpt-oss-120b-medium': { name: 'GPT-OSS 120B (Medium)', shortcut: '6' },
};

// ── Helpers ──

function parseRecords(text: string): UsageRecord[] {
    const lines = text.trim().split('\n');
    const records: UsageRecord[] = [];
    for (const line of lines) {
        const trimmed = line.trim();
        if (!trimmed) continue;
        try {
            records.push(JSON.parse(trimmed));
        } catch {
            // skip malformed lines
        }
    }
    return records;
}

function parseTimestamp(ts: string): Date {
    // "2026-03-21 21:53:39" → Date
    return new Date(ts.replace(' ', 'T'));
}

function getDateStr(timestamp: string): string {
    return timestamp.split(' ')[0];
}

function getTodayStr(): string {
    const now = new Date();
    const y = now.getFullYear();
    const m = String(now.getMonth() + 1).padStart(2, '0');
    const d = String(now.getDate()).padStart(2, '0');
    return `${y}-${m}-${d}`;
}

function filterByTimeRange(records: UsageRecord[], range: TimeRange): UsageRecord[] {
    const now = Date.now();
    if (range === '10min') {
        const cutoff = now - 10 * 60 * 1000;
        return records.filter((r) => parseTimestamp(r.timestamp).getTime() >= cutoff);
    }
    if (range === '1h') {
        const cutoff = now - 60 * 60 * 1000;
        return records.filter((r) => parseTimestamp(r.timestamp).getTime() >= cutoff);
    }
    if (range === '1day') {
        const today = getTodayStr();
        return records.filter((r) => getDateStr(r.timestamp) === today);
    }
    // 'overall' — all records
    return records;
}

function groupByGranularity(records: UsageRecord[], range: TimeRange): ChartPoint[] {
    if (range === '10min') {
        // Each record as a data point, label = HH:MM:SS
        return records.map((r) => ({
            label: r.timestamp.split(' ')[1] ?? r.timestamp,
            cost: r.total_cost_est,
        }));
    }
    if (range === '1h') {
        // Group by 10-minute bucket, label = HH:MM
        const map = new Map<string, number>();
        for (const r of records) {
            const time = r.timestamp.split(' ')[1] ?? '00:00:00';
            const [h, m] = time.split(':');
            const bucket = `${h}:${String(Math.floor(Number(m) / 10) * 10).padStart(2, '0')}`;
            map.set(bucket, (map.get(bucket) ?? 0) + r.total_cost_est);
        }
        return Array.from(map.entries())
            .map(([label, cost]) => ({ label, cost }))
            .sort((a, b) => a.label.localeCompare(b.label));
    }
    if (range === '1day') {
        // Group by hour within today, label = HH:00
        const map = new Map<string, number>();
        for (const r of records) {
            const time = r.timestamp.split(' ')[1] ?? '00:00:00';
            const h = time.split(':')[0];
            const bucket = `${h}:00`;
            map.set(bucket, (map.get(bucket) ?? 0) + r.total_cost_est);
        }
        return Array.from(map.entries())
            .map(([label, cost]) => ({ label, cost }))
            .sort((a, b) => a.label.localeCompare(b.label));
    }
    // 'overall' — group by date
    const map = new Map<string, number>();
    for (const r of records) {
        const date = getDateStr(r.timestamp);
        map.set(date, (map.get(date) ?? 0) + r.total_cost_est);
    }
    return Array.from(map.entries())
        .map(([label, cost]) => ({ label: shortDate(label), cost }))
        .sort((a, b) => a.label.localeCompare(b.label));
}

function groupByModel(records: UsageRecord[]): ModelSummary[] {
    const map = new Map<string, ModelSummary>();
    for (const r of records) {
        const existing = map.get(r.model);
        if (existing) {
            existing.inputTokens += r.input_tokens_est;
            existing.outputTokens += r.output_tokens_est;
            existing.cost += r.total_cost_est;
            existing.records += 1;
        } else {
            map.set(r.model, {
                model: r.model,
                inputTokens: r.input_tokens_est,
                outputTokens: r.output_tokens_est,
                cost: r.total_cost_est,
                records: 1,
            });
        }
    }
    return Array.from(map.values()).sort((a, b) => b.cost - a.cost);
}

function formatCost(v: number): string {
    return `$${v.toFixed(2)}`;
}

function formatTokens(v: number): string {
    if (v >= 1_000_000) return `${(v / 1_000_000).toFixed(1)}M`;
    if (v >= 1_000) return `${(v / 1_000).toFixed(1)}K`;
    return String(v);
}

function shortDate(date: string): string {
    return date.slice(5);
}

const TIME_RANGE_LABELS: Record<TimeRange, string> = {
    '10min': '10 min',
    '1h': '1 hour',
    '1day': 'Today',
    'overall': 'Overall',
};

const CHART_TITLES: Record<TimeRange, string> = {
    '10min': 'Cost (Last 10 min)',
    '1h': 'Cost (Last Hour, 10min buckets)',
    '1day': 'Today by Hour',
    'overall': 'Daily Cost Trend',
};

// ── Chart ──

function CostChart({ data }: { data: ChartPoint[] }) {
    const [tooltip, setTooltip] = useState<{
        x: number;
        y: number;
        date: string;
        cost: number;
    } | null>(null);

    if (data.length === 0) {
        return (
            <div className="tm-empty-state">
                <div className="tm-empty-icon">
                    <TrendingUp size={28} />
                </div>
                <div className="tm-empty-title">No chart data</div>
                <div className="tm-empty-desc">Usage data will appear here once the estimator starts recording.</div>
            </div>
        );
    }

    const padL = 50;
    const padR = 16;
    const padT = 16;
    const padB = 32;
    const W = 800;
    const H = 240;
    const plotW = W - padL - padR;
    const plotH = H - padT - padB;

    const maxCost = Math.max(...data.map((d) => d.cost), 0.01);
    const yMax = Math.ceil(maxCost * 1.15 * 100) / 100;

    const xScale = (i: number) => padL + (plotW * i) / Math.max(data.length - 1, 1);
    const yScale = (v: number) => padT + plotH - (plotH * v) / yMax;

    const points = data.map((d, i) => `${xScale(i)},${yScale(d.cost)}`).join(' ');
    const areaPoints = `${xScale(0)},${yScale(0)} ${points} ${xScale(data.length - 1)},${yScale(0)}`;

    const yTicks = [0, 0.25, 0.5, 0.75, 1].map((f) => f * yMax);
    const step = Math.max(1, Math.ceil(data.length / 12));

    return (
        <div className="tm-chart-svg-container">
            <svg className="tm-chart-svg" viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="none">
                <defs>
                    <linearGradient id="chartGradient" x1="0" y1="0" x2="0" y2="1">
                        <stop offset="0%" stopColor="#f59e0b" stopOpacity="0.4" />
                        <stop offset="100%" stopColor="#f59e0b" stopOpacity="0" />
                    </linearGradient>
                </defs>

                {yTicks.map((tick, i) => (
                    <g key={i}>
                        <line x1={padL} y1={yScale(tick)} x2={W - padR} y2={yScale(tick)} className="tm-chart-grid-line" />
                        <text x={padL - 8} y={yScale(tick) + 4} className="tm-chart-y-label">
                            ${tick.toFixed(2)}
                        </text>
                    </g>
                ))}

                <polygon points={areaPoints} className="tm-chart-area" />
                <polyline points={points} className="tm-chart-line" />

                {data.map((d, i) => (
                    <circle
                        key={i}
                        cx={xScale(i)}
                        cy={yScale(d.cost)}
                        r={3.5}
                        className="tm-chart-dot"
                        onMouseEnter={() =>
                            setTooltip({ x: xScale(i), y: yScale(d.cost), date: d.label, cost: d.cost })
                        }
                        onMouseLeave={() => setTooltip(null)}
                    />
                ))}

                {data.map((d, i) =>
                    i % step === 0 || i === data.length - 1 ? (
                        <text key={i} x={xScale(i)} y={H - 4} className="tm-chart-x-label">
                            {d.label}
                        </text>
                    ) : null,
                )}
            </svg>

            {tooltip && (
                <div
                    className="tm-chart-tooltip"
                    style={{
                        left: `${(tooltip.x / W) * 100}%`,
                        top: `${(tooltip.y / H) * 100}%`,
                    }}
                >
                    <div className="tm-chart-tooltip-date">{tooltip.date}</div>
                    <div className="tm-chart-tooltip-value">{formatCost(tooltip.cost)}</div>
                </div>
            )}
        </div>
    );
}

// ── Main ──

const POLL_INTERVAL = 20_000;
const CONFIG_DIR_REL = ['.config', 'anti-tracker'] as const;

export function TokenMonitorPage() {
    const [records, setRecords] = useState<UsageRecord[]>([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const [lastUpdate, setLastUpdate] = useState<Date | null>(null);
    const [timeRange, setTimeRange] = useState<TimeRange>('1day');
    const [currentModel, setCurrentModel] = useState<string>('claude-opus-4.6-thinking');
    const [modelDropdownOpen, setModelDropdownOpen] = useState(false);
    const [modelSaving, setModelSaving] = useState(false);
    const configDirRef = useRef<string | null>(null);
    const filePathRef = useRef<string | null>(null);

    const getConfigDir = useCallback(async () => {
        if (!configDirRef.current) {
            const home = await homeDir();
            configDirRef.current = await join(home, ...CONFIG_DIR_REL);
        }
        return configDirRef.current;
    }, []);

    const loadData = useCallback(async () => {
        try {
            if (!filePathRef.current) {
                const dir = await getConfigDir();
                filePathRef.current = await join(dir, 'nettop_usage.jsonl');
            }
            const text = await readTextFile(filePathRef.current);
            const parsed = parseRecords(text);
            setRecords(parsed);
            setError(null);
            setLastUpdate(new Date());
        } catch (e: unknown) {
            const msg = e instanceof Error ? e.message : String(e);
            if (msg.includes('not found') || msg.includes('No such file') || msg.includes('os error 2') || msg.includes('os error 3')) {
                setRecords([]);
                setError(null);
            } else {
                setError(msg);
            }
        } finally {
            setLoading(false);
        }
    }, [getConfigDir]);

    // Load current model from config
    const loadCurrentModel = useCallback(async () => {
        try {
            const dir = await getConfigDir();
            const modelFile = await join(dir, 'current_model.json');
            const text = await readTextFile(modelFile);
            const data = JSON.parse(text);
            if (data.model && data.model in MODEL_CATALOG) {
                setCurrentModel(data.model);
            }
        } catch {
            // File doesn't exist or unreadable — use default
        }
    }, [getConfigDir]);

    const switchModel = useCallback(async (modelId: string) => {
        setModelSaving(true);
        try {
            const dir = await getConfigDir();
            const modelFile = await join(dir, 'current_model.json');
            const data = {
                model: modelId,
                changed_at: new Date().toISOString().replace('T', ' ').slice(0, 19),
            };
            await writeTextFile(modelFile, JSON.stringify(data));
            setCurrentModel(modelId);
        } catch (e: unknown) {
            const msg = e instanceof Error ? e.message : String(e);
            setError(`Failed to save model: ${msg}`);
        } finally {
            setModelSaving(false);
            setModelDropdownOpen(false);
        }
    }, [getConfigDir]);

    useEffect(() => {
        loadData();
        loadCurrentModel();
        const timer = setInterval(loadData, POLL_INTERVAL);
        return () => clearInterval(timer);
    }, [loadData, loadCurrentModel]);

    // Close dropdown on outside click
    useEffect(() => {
        if (!modelDropdownOpen) return;
        const handler = () => setModelDropdownOpen(false);
        const timer = setTimeout(() => document.addEventListener('click', handler), 0);
        return () => {
            clearTimeout(timer);
            document.removeEventListener('click', handler);
        };
    }, [modelDropdownOpen]);

    const handleOpenFile = useCallback(async () => {
        try {
            const dir = await getConfigDir();
            await openPath(dir);
        } catch {
            // silently ignore
        }
    }, [getConfigDir]);

    const handleRefresh = useCallback(() => {
        setLoading(true);
        loadData();
    }, [loadData]);

    // ── Compute derived data ──
    const filteredRecords = useMemo(() => filterByTimeRange(records, timeRange), [records, timeRange]);
    const filteredCost = filteredRecords.reduce((sum, r) => sum + r.total_cost_est, 0);
    const filteredInput = filteredRecords.reduce((sum, r) => sum + r.input_tokens_est, 0);
    const filteredOutput = filteredRecords.reduce((sum, r) => sum + r.output_tokens_est, 0);

    const latestRecord = records.length > 0 ? records[records.length - 1] : null;
    const dataSource = latestRecord?.source ?? 'unknown';

    const chartData = useMemo(() => {
        const src = timeRange === 'overall' ? records : filteredRecords;
        return groupByGranularity(src, timeRange);
    }, [records, filteredRecords, timeRange]);
    const modelSummaries = useMemo(() => groupByModel(filteredRecords), [filteredRecords]);

    const currentModelName = MODEL_CATALOG[currentModel]?.name ?? currentModel;

    if (loading && records.length === 0) {
        return (
            <div className="token-monitor-page">
                <div className="tm-loading">
                    <div className="tm-loading-spinner" />
                    Loading usage data...
                </div>
            </div>
        );
    }

    return (
        <div className="token-monitor-page">
            {/* Header */}
            <div className="token-monitor-header">
                <div className="token-monitor-title">
                    <div className="token-monitor-title-icon">
                        <Zap size={20} />
                    </div>
                    Token Monitor
                </div>
                <div className="token-monitor-actions">
                    <span className={`tm-source-badge ${dataSource === 'clash' ? 'precise' : 'approx'}`}>
                        <Radio size={12} />
                        {dataSource === 'clash' ? 'Clash (Precise)' : dataSource === 'psutil' ? 'psutil (Approx)' : 'No Data'}
                    </span>
                    {lastUpdate && (
                        <span className="token-monitor-last-update">
                            Updated {lastUpdate.toLocaleTimeString()}
                        </span>
                    )}
                    <button className="token-monitor-btn" onClick={handleRefresh}>
                        <RefreshCw size={14} />
                        Refresh
                    </button>
                    <button className="token-monitor-btn" onClick={handleOpenFile}>
                        <FolderOpen size={14} />
                        Open Raw File
                    </button>
                </div>
            </div>

            {/* Error */}
            {error && (
                <div className="tm-error-banner">
                    <AlertCircle size={16} />
                    {error}
                </div>
            )}

            {/* Model Selector + Time Range Tabs */}
            <div className="tm-controls-row">
                {/* Model Selector */}
                <div className="tm-model-selector" onClick={(e) => e.stopPropagation()}>
                    <button
                        className="tm-model-trigger"
                        onClick={() => setModelDropdownOpen(!modelDropdownOpen)}
                        disabled={modelSaving}
                    >
                        <Zap size={14} />
                        <span className="tm-model-current">{currentModelName}</span>
                        <ChevronDown size={14} className={modelDropdownOpen ? 'rotated' : ''} />
                    </button>
                    {modelDropdownOpen && (
                        <div className="tm-model-dropdown">
                            {Object.entries(MODEL_CATALOG).map(([id, m]) => (
                                <button
                                    key={id}
                                    className={`tm-model-option ${id === currentModel ? 'active' : ''}`}
                                    onClick={() => switchModel(id)}
                                >
                                    <span className="tm-model-option-shortcut">{m.shortcut}</span>
                                    <span className="tm-model-option-name">{m.name}</span>
                                    {id === currentModel && <span className="tm-model-option-check">✓</span>}
                                </button>
                            ))}
                            <div className="tm-model-dropdown-hint">
                                ⚠ Restart estimator after switching
                            </div>
                        </div>
                    )}
                </div>

                {/* Time Range Tabs */}
                <div className="tm-time-tabs">
                    <Clock size={14} className="tm-time-tabs-icon" />
                    {(['10min', '1h', '1day', 'overall'] as TimeRange[]).map((range) => (
                        <button
                            key={range}
                            className={`tm-time-tab ${timeRange === range ? 'active' : ''}`}
                            onClick={() => setTimeRange(range)}
                        >
                            {TIME_RANGE_LABELS[range]}
                        </button>
                    ))}
                </div>
            </div>

            {/* Summary Cards */}
            <div className="tm-cards-row">
                <div className="tm-card">
                    <div className="tm-card-icon cost">
                        <DollarSign size={24} />
                    </div>
                    <div className="tm-card-info">
                        <div className="tm-card-value cost-value">{formatCost(filteredCost)}</div>
                        <div className="tm-card-label">Cost ({TIME_RANGE_LABELS[timeRange]})</div>
                    </div>
                </div>

                <div className="tm-card">
                    <div className="tm-card-icon input">
                        <ArrowDownToLine size={24} />
                    </div>
                    <div className="tm-card-info">
                        <div className="tm-card-value">{formatTokens(filteredInput)}</div>
                        <div className="tm-card-label">Input Tokens</div>
                    </div>
                </div>

                <div className="tm-card">
                    <div className="tm-card-icon output">
                        <ArrowUpFromLine size={24} />
                    </div>
                    <div className="tm-card-info">
                        <div className="tm-card-value">{formatTokens(filteredOutput)}</div>
                        <div className="tm-card-label">Output Tokens</div>
                    </div>
                </div>
            </div>

            <div className="tm-chart-card">
                <div className="tm-chart-header">
                    <div className="tm-chart-title">
                        <TrendingUp size={18} />
                        {CHART_TITLES[timeRange]}
                    </div>
                </div>
                <div className="tm-chart-body">
                    <CostChart data={chartData} />
                </div>
            </div>

            {/* Model Breakdown Table */}
            <div className="tm-table-card">
                <div className="tm-table-header">
                    <div className="tm-table-title">
                        <BarChart3 size={18} />
                        By Model ({TIME_RANGE_LABELS[timeRange]})
                    </div>
                    <span className="tm-table-count">{filteredRecords.length} records</span>
                </div>
                <div className="tm-table-body">
                    {modelSummaries.length === 0 ? (
                        <div className="tm-empty-state">
                            <div className="tm-empty-icon">
                                <FileQuestion size={28} />
                            </div>
                            <div className="tm-empty-title">No data for {TIME_RANGE_LABELS[timeRange].toLowerCase()}</div>
                            <div className="tm-empty-desc">
                                Token usage data will appear here when the estimator starts recording.
                            </div>
                        </div>
                    ) : (
                        <table className="tm-table">
                            <thead>
                                <tr>
                                    <th>Model</th>
                                    <th className="right">Input Tokens</th>
                                    <th className="right">Output Tokens</th>
                                    <th className="right">Records</th>
                                    <th className="right">Cost</th>
                                </tr>
                            </thead>
                            <tbody>
                                {modelSummaries.map((m) => (
                                    <tr key={m.model}>
                                        <td>
                                            <span className="tm-model-name">{MODEL_CATALOG[m.model]?.name ?? m.model}</span>
                                        </td>
                                        <td className="right">{formatTokens(m.inputTokens)}</td>
                                        <td className="right">{formatTokens(m.outputTokens)}</td>
                                        <td className="right">{m.records}</td>
                                        <td className="right cost">{formatCost(m.cost)}</td>
                                    </tr>
                                ))}
                                {modelSummaries.length > 1 && (
                                    <tr>
                                        <td>
                                            <span className="tm-model-name" style={{ fontWeight: 700 }}>
                                                Total
                                            </span>
                                        </td>
                                        <td className="right" style={{ fontWeight: 700 }}>
                                            {formatTokens(filteredInput)}
                                        </td>
                                        <td className="right" style={{ fontWeight: 700 }}>
                                            {formatTokens(filteredOutput)}
                                        </td>
                                        <td className="right" style={{ fontWeight: 700 }}>
                                            {filteredRecords.length}
                                        </td>
                                        <td className="right cost" style={{ fontWeight: 700 }}>
                                            {formatCost(filteredCost)}
                                        </td>
                                    </tr>
                                )}
                            </tbody>
                        </table>
                    )}
                </div>
            </div>
        </div>
    );
}
