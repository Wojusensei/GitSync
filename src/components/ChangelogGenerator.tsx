import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { motion } from 'framer-motion';

interface ChangelogEntry {
  version: string;
  date: string;
  messages: string[];
}

export default function ChangelogGenerator({ repoPath }: { repoPath: string }) {
  const [entries, setEntries] = useState<ChangelogEntry[]>([]);
  const [loading, setLoading] = useState(false);

  const generate = async () => {
    setLoading(true);
    try {
      const res = await invoke<ChangelogEntry[]>('generate_changelog', { path: repoPath, count: 100 });
      setEntries(res);
    } catch (e) { console.error(e); }
    finally { setLoading(false); }
  };

  const copyToClipboard = () => {
    let md = "# Changelog\n\n";
    entries.forEach(e => {
      md += `## ${e.date}\n`;
      e.messages.forEach(m => md += `- ${m}\n`);
      md += "\n";
    });
    navigator.clipboard.writeText(md);
    alert('Changelog 已复制到剪贴板');
  };

  return (
    <motion.div className="analysis-panel" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }}>
      <h3>生成变更日志</h3>
      <button className="btn btn-blue" onClick={generate} disabled={loading} style={{ marginBottom: 12 }}>生成</button>
      {entries.map((e, i) => (
        <div key={i} className="analysis-item">
          <span className="section-title">{e.date}</span>
          {e.messages.map((m, j) => <div key={j} className="message">- {m}</div>)}
        </div>
      ))}
      {entries.length > 0 && <button className="btn btn-blue" onClick={copyToClipboard}>复制到剪贴板</button>}
    </motion.div>
  );
}