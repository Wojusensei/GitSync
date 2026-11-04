import { useState } from 'react';
import { motion } from 'framer-motion';
import { invokeTauri } from '../services/tauriService';

const MAX_DIFF_LINES = 3000;

export default function DiffViewer({ repoPath }: { repoPath: string }) {
  const [commitA, setCommitA] = useState('');
  const [commitB, setCommitB] = useState('');
  const [diff, setDiff] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');

  const handleCompare = async () => {
    setLoading(true);
    setError('');
    setDiff('');
    try {
      const res: any = await invokeTauri('compare_commits', { path: repoPath, commitA, commitB });
      setDiff(res.diff);
    } catch (e: any) {
      setError(String(e));
    } finally { setLoading(false); }
  };

  const swap = () => {
    setCommitA(commitB);
    setCommitB(commitA);
  };

  const lines = diff ? diff.split('\n') : [];
  const shown = lines.slice(0, MAX_DIFF_LINES);
  const added = lines.filter(l => l.startsWith('+') && !l.startsWith('+++')).length;
  const removed = lines.filter(l => l.startsWith('-') && !l.startsWith('---')).length;

  const lineClass = (line: string) => {
    if (line.startsWith('@@')) return 'diff-line hunk';
    if (line.startsWith('+++') || line.startsWith('---')) return 'diff-line meta';
    if (line.startsWith('diff --git') || line.startsWith('index ')) return 'diff-line meta';
    if (line.startsWith('+')) return 'diff-line add';
    if (line.startsWith('-')) return 'diff-line del';
    return 'diff-line';
  };

  return (
    <motion.div className="analysis-panel" initial={{ opacity: 0, y: 12 }} animate={{ opacity: 1, y: 0 }}>
      <div className="panel-title-row">
        <h3>可视化差异对比</h3>
        {diff && (
          <span className="panel-hint">
            <span style={{ color: 'var(--diff-add-text)' }}>+{added}</span>
            {' / '}
            <span style={{ color: 'var(--diff-del-text)' }}>-{removed}</span>
          </span>
        )}
      </div>
      <div className="diff-compare-bar">
        <label className="filter-field">
          <span>提交 A（旧）</span>
          <input className="filter-input" value={commitA} onChange={e => setCommitA(e.target.value)} placeholder="哈希或 ref" />
        </label>
        <button className="diff-swap" onClick={swap} title="交换 A / B" type="button">⇄</button>
        <label className="filter-field">
          <span>提交 B（新）</span>
          <input className="filter-input" value={commitB} onChange={e => setCommitB(e.target.value)} placeholder="哈希或 ref" />
        </label>
        <button className="btn btn-blue" onClick={handleCompare} disabled={loading || !commitA.trim() || !commitB.trim()}>
          {loading ? '对比中…' : '对比'}
        </button>
      </div>
      {error && <div className="analysis-item" style={{ color: 'var(--danger)' }}>{error}</div>}
      {diff && (
        <div className="diff-view">
          {shown.map((line, i) => (
            <div key={i} className={lineClass(line)}>{line || ' '}</div>
          ))}
          {lines.length > MAX_DIFF_LINES && (
            <div className="diff-line meta">… 只显示前 {MAX_DIFF_LINES} 行，共 {lines.length} 行</div>
          )}
        </div>
      )}
    </motion.div>
  );
}
