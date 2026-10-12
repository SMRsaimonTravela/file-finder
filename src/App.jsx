import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

const EXTS = ["", "pdf", "docx", "xlsx", "txt", "jpg", "png", "mp4", "mp3", "zip", "exe"];

export default function App() {
  const [q, setQ] = useState("");
  const [ext, setExt] = useState("");
  const [items, setItems] = useState([]);
  const [sel, setSel] = useState(0);
  const [status, setStatus] = useState({ indexing: false, count: 0 });
  const [theme, setTheme] = useState(localStorage.getItem("theme") || "dark");
  const input = useRef(null);

  useEffect(() => { document.documentElement.dataset.theme = theme; localStorage.setItem("theme", theme); }, [theme]);

  useEffect(() => {
    input.current?.focus();
    const un = listen("focus-search", () => { input.current?.focus(); input.current?.select(); });
    const t = setInterval(() => invoke("status").then(setStatus), 1000);
    return () => { un.then((f) => f()); clearInterval(t); };
  }, []);

  useEffect(() => {
    const t = setTimeout(async () => {
      if (!q.trim()) { setItems([]); return; }
      const r = await invoke("search", { query: q, ext: ext || null, limit: 100 });
      setItems(r); setSel(0);
    }, 60);
    return () => clearTimeout(t);
  }, [q, ext, status.count > 0 && !status.indexing]);

  const open = (it) => invoke("open_path", { path: it.path });
  const reveal = (it) => invoke("reveal_path", { path: it.path });
  const copy = (it) => navigator.clipboard.writeText(it.path);

  const onKey = (e) => {
    const it = items[sel];
    if (e.key === "ArrowDown") { e.preventDefault(); setSel((s) => Math.min(s + 1, items.length - 1)); }
    else if (e.key === "ArrowUp") { e.preventDefault(); setSel((s) => Math.max(s - 1, 0)); }
    else if (e.key === "Enter" && it) { e.ctrlKey ? reveal(it) : open(it); }
    else if (e.key === "c" && e.ctrlKey && it && window.getSelection().toString() === "") { copy(it); }
    else if (e.key === "Escape") { invoke("hide_window"); }
  };

  return (
    <div className="app" onKeyDown={onKey}>
      <div className="bar">
        <input ref={input} value={q} onChange={(e) => setQ(e.target.value)} placeholder="ফাইল বা ফোল্ডারের নাম লিখুন…" spellCheck={false} />
        <select value={ext} onChange={(e) => setExt(e.target.value)}>
          {EXTS.map((x) => <option key={x} value={x}>{x ? "." + x : "সব"}</option>)}
        </select>
        <button onClick={() => setTheme(theme === "dark" ? "light" : "dark")}>{theme === "dark" ? "☀️" : "🌙"}</button>
      </div>
      <div className="list">
        {items.map((it, i) => (
          <div key={it.path} className={"row" + (i === sel ? " sel" : "")} onMouseEnter={() => setSel(i)} onDoubleClick={() => open(it)}>
            <span className="icon">{it.is_dir ? "📁" : "📄"}</span>
            <div className="txt"><div className="name">{it.name}</div><div className="path">{it.path}</div></div>
            <div className="acts">
              <button onClick={() => open(it)}>Open</button>
              <button onClick={() => reveal(it)}>Folder</button>
              <button onClick={() => copy(it)}>Copy</button>
            </div>
          </div>
        ))}
        {q && items.length === 0 && <div className="empty">কিছু পাওয়া যায়নি</div>}
      </div>
      <div className="foot">
        {status.indexing ? `⏳ Indexing… ${status.count.toLocaleString()} টি আইটেম` : `✅ ${status.count.toLocaleString()} টি আইটেম index করা`}
        <span>↑↓ বাছাই · Enter খুলুন · Ctrl+Enter ফোল্ডার · Ctrl+C পাথ কপি · Esc লুকান</span>
      </div>
    </div>
  );
}
