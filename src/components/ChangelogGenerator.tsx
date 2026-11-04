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
  const [error, setError] = useState('');

  const generate = async () => {
    setLoading(true);
    setError('');
    try {
      const res = await invoke<ChangelogEntry[]>('generate_changelog', { path: repoPath, count: 100 });
      setEntries(res);
    } catch (e: any) {
      setError(String(e));
    } finally { setLoading(false); }
  };

  const copyToClipboard = async () => {
    let md = "# Changelog\n\n";
    entries.forEach(e => {
      md += `## ${e.date}\n`;
      e.messages.forEach(m => md += `- ${m.split('\n')[0]}\n`);
      md += "\n";
    });
    try {
      await navigator.clipboard.writeText(md);
      alert('Changelog 已复制到剪贴板');
    } catch {
      setError('复制到剪贴板失败，请重试');
    }
  };

  return (
    <motion.div className="analysis-panel" initial={{ opacity: 0, y: 12 }} animate={{ opacity: 1, y: 0 }}>
      <h3>生成变更日志</h3>
      <button className="btn btn-blue" onClick={generate} disabled={loading} style={{ marginBottom: 12 }}>{loading ? '生成中...' : '生成'}</button>
      {error && <div className="analysis-item" style={{ color: 'var(--danger)' }}>{error}</div>}
      {entries.map((e, i) => (
        <div key={i} className="changelog-group">
          <div className="changelog-date">{e.date}</div>
          <ul className="changelog-list">
            {e.messages.map((m, j) => <li key={j}>{m.split('\n')[0]}</li>)}
          </ul>
        </div>
      ))}
      {entries.length > 0 && <button className="btn btn-blue" onClick={copyToClipboard}>复制到剪贴板</button>}
    </motion.div>
  );
}
