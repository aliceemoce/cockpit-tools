from pathlib import Path
p=Path("src-tauri/src/commands/cursor_instance.rs")
t=p.read_text(encoding="utf-8")
t=t.replace("Result<(String, &'static str, i32), String>", "Result<(String, &'static str, usize), String>")
t=t.replace("return Ok((full_id, \"full\", 1));", "return Ok((full_id, \"full\", 1usize));")
t=t.replace("Ok((id, \"highest\", 1))", "Ok((id, \"highest\", 1usize))")
p.write_text(t, encoding="utf-8")
print("fixed")
