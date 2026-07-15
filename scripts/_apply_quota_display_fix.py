from pathlib import Path
PAGE = Path("src/pages/CursorAccountsPage.tsx")
text = PAGE.read_text(encoding="utf-8")
marker_start = "  const resolveTotalQuota = useCallback("
marker_end = "  const resolveOnDemandQuota = useCallback("
i0 = text.index(marker_start)
i1 = text.index(marker_end)
new_display = """  const resolveTotalQuota = useCallback(
    (account: CursorAccount) => {
      const usage = getCursorUsage(account);
      const ratioPct =
        usage.planUsedCents != null &&
        usage.planLimitCents != null &&
        usage.planLimitCents > 0
          ? (usage.planUsedCents / usage.planLimitCents) * 100
          : null;
      const total = normalizeCursorPercent(usage.totalPercentUsed ?? ratioPct);
      const costText = usage.planUsedCents != null && usage.planLimitCents != null
        ? `${formatCursorUsageDollars(usage.planUsedCents)} / ${formatCursorUsageDollars(usage.planLimitCents)}`
        : null;
      return {
        percentage: total.bar,
        quotaClass: getCursorQuotaClass(total.display),
        valueText: `${total.display}%`,
        costText,
      };
    },
    [],
  );

  const resolveAutoQuota = useCallback(
    (account: CursorAccount) => {
      const usage = getCursorUsage(account);
      const auto = normalizeCursorPercent(usage.autoPercentUsed);
      return {
        percentage: auto.bar,
        quotaClass: getCursorQuotaClass(auto.display),
        valueText: `${auto.display}%`,
      };
    },
    [],
  );

  const resolveApiQuota = useCallback(
    (account: CursorAccount) => {
      const usage = getCursorUsage(account);
      const api = normalizeCursorPercent(usage.apiPercentUsed);
      return {
        percentage: api.bar,
        quotaClass: getCursorQuotaClass(api.display),
        valueText: `${api.display}%`,
      };
    },
    [],
  );

"""
text = text[:i0] + new_display + text[i1:]
rr_old_start = "  const resolveRemainingQuotaPercent = useCallback"
rr_old_end = "  const compareUsageUpdatedAt = useCallback"
j0 = text.index(rr_old_start)
j1 = text.index(rr_old_end)
new_rr = """  /** sort-only RR (36786727); display stays 590/upstream */
  const resolveRemainingQuotaPercent = useCallback((account: CursorAccount): number | null => {
    if (account.quota_query_last_error?.trim()) {
      return null;
    }
    const usage = getCursorUsage(account);
    const ratioPct =
      usage.planUsedCents != null &&
      usage.planLimitCents != null &&
      usage.planLimitCents > 0
        ? (usage.planUsedCents / usage.planLimitCents) * 100
        : null;
    const usageDims = [
      usage.autoPercentUsed,
      usage.apiPercentUsed,
      usage.totalPercentUsed,
      usage.inlineSuggestionsUsedPercent,
    ];
    const updatedAt = account.usage_updated_at ?? 0;
    const staleMs = 24 * 60 * 60 * 1000;
    const isStale = updatedAt > 0 && Date.now() - updatedAt * 1000 > staleMs;
    const allZeroOrNull = usageDims.every((value) => value == null || value === 0);
    if (isStale && allZeroOrNull) {
      return null;
    }
    const usedCandidates = [
      usage.inlineSuggestionsUsedPercent ?? usage.totalPercentUsed ?? ratioPct,
      usage.autoPercentUsed,
      usage.apiPercentUsed,
    ].filter((value): value is number => value != null && Number.isFinite(value));
    if (usedCandidates.length === 0) {
      return null;
    }
    const maxUsed = Math.min(100, Math.max(0, Math.max(...usedCandidates)));
    return 100 - maxUsed;
  }, []);

"""
text = text[:j0] + new_rr + text[j1:]
PAGE.write_text(text, encoding="utf-8")
print("ok page")
