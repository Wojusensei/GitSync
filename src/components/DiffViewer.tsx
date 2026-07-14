import { useState } from 'react';
import { motion } from 'framer-motion';
import { invokeTauri } from '../services/tauriService';

export default function DiffViewer({ repoPath }: { repoPath: string }) {
  const [commitA, setCommitA] = useState('');
  const [commitB, setCommitB] = useState('');
  const [diff, setDiff] = useState('');
  const [loading, setLoading] = useState(false);

  const handleCompare = async () => {
    setLoading(true);
    try {
      const res: any = await invokeTauri('compare_commits', { path: repoPath, commitA, commitB });
      setDiff(res.diff);
    } catch (e) { console.error(e); }
    finally { setLoading(false); }
  };

  return (
    <motion.div className="analysis-panel" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }}>
      <h3>可视化差异对比</h3>
      <div style={{ display: 'flex', gap: 8, marginBottom: 12 }}>
        <input className="path-input" value={commitA} onChange={e => setCommitA(e.target.value)} placeholder="提交 A 哈希" />
        <input className="path-input" value={commitB} onChange={e => setCommitB(e.target.value)} placeholder="提交 B 哈希" />
        <button className="btn btn-blue" onClick={handleCompare} disabled={loading}>对比</button>
      </div>
      <pre className="diff-content">{diff}</pre>
    </motion.div>
  );
}