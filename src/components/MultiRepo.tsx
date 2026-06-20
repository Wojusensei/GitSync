import { useState } from 'react';
import { motion } from 'framer-motion';

export default function MultiRepo({ onSelectRepo }: { onSelectRepo: (path: string) => void }) {
  const [paths, setPaths] = useState<string[]>([]);
  const [newPath, setNewPath] = useState('');

  const addRepo = () => {
    if (newPath && !paths.includes(newPath)) {
      setPaths([...paths, newPath]);
      setNewPath('');
    }
  };

  return (
    <motion.div className="analysis-panel" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }}>
      <h3>多仓库管理</h3>
      <div style={{ display: 'flex', gap: 8, marginBottom: 12 }}>
        <input className="path-input" value={newPath} onChange={e => setNewPath(e.target.value)} placeholder="仓库路径" />
        <button className="btn btn-blue" onClick={addRepo}>添加</button>
      </div>
      {paths.map(p => (
        <div key={p} className="analysis-item" onClick={() => onSelectRepo(p)} style={{ cursor: 'pointer' }}>
          {p}
        </div>
      ))}
    </motion.div>
  );
}