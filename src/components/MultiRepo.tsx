import { useState } from 'react';
import { motion } from 'framer-motion';

const STORAGE_KEY = 'gitsync.recent_repos';

function loadPaths(): string[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    const parsed = raw ? JSON.parse(raw) : [];
    return Array.isArray(parsed) ? parsed.filter((p: unknown) => typeof p === 'string') : [];
  } catch {
    return [];
  }
}

export default function MultiRepo({ onSelectRepo }: { onSelectRepo: (path: string) => void }) {
  // 仓库列表持久化：此前是组件局部 state，面板一关就清空
  const [paths, setPaths] = useState<string[]>(loadPaths);
  const [newPath, setNewPath] = useState('');

  const addRepo = () => {
    const trimmed = newPath.trim();
    if (trimmed && !paths.includes(trimmed)) {
      const next = [...paths, trimmed];
      setPaths(next);
      try { localStorage.setItem(STORAGE_KEY, JSON.stringify(next)); } catch { /* 存储不可用则仅本次生效 */ }
      setNewPath('');
    }
  };

  const removeRepo = (p: string) => {
    const next = paths.filter(x => x !== p);
    setPaths(next);
    try { localStorage.setItem(STORAGE_KEY, JSON.stringify(next)); } catch { /* 同上 */ }
  };

  return (
    <motion.div className="analysis-panel" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }}>
      <h3>多仓库管理</h3>
      <div style={{ display: 'flex', gap: 8, marginBottom: 12 }}>
        <input className="path-input" value={newPath} onChange={e => setNewPath(e.target.value)} placeholder="仓库路径" onKeyDown={e => e.key === 'Enter' && addRepo()} />
        <button className="btn btn-blue" onClick={addRepo}>添加</button>
      </div>
      {paths.length === 0 && <div style={{ color: 'var(--text-dim)', fontSize: 12, padding: '0 6px' }}>暂无记录，添加后跨会话保留</div>}
      {paths.map(p => (
        <div key={p} className="analysis-item" style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <span onClick={() => onSelectRepo(p)} style={{ cursor: 'pointer', flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
            {p}
          </span>
          <button
            className="detail-action-btn"
            onClick={() => removeRepo(p)}
            title="从列表移除"
            style={{ flexShrink: 0 }}
          >×</button>
        </div>
      ))}
    </motion.div>
  );
}
