import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { motion } from 'framer-motion';

export default function CommitFilter({ repoPath, onFiltered, onReset }: {
  repoPath: string;
  onFiltered: (commits: any[]) => void;
  onReset?: () => void;
}) {
  const [author, setAuthor] = useState('');
  const [dateFrom, setDateFrom] = useState('');
  const [dateTo, setDateTo] = useState('');
  const [filePath, setFilePath] = useState('');
  const [allRefs, setAllRefs] = useState(false);
  const [loading, setLoading] = useState(false);
  const [hitCount, setHitCount] = useState<number | null>(null);
  const [error, setError] = useState('');

  const hasCondition = Boolean(author || dateFrom || dateTo || filePath || allRefs);

  const applyFilter = async (e?: React.FormEvent) => {
    e?.preventDefault();
    setError('');
    setLoading(true);
    try {
      const res = await invoke<any[]>('filter_commits', {
        path: repoPath, author: author || null, dateFrom: dateFrom || null, dateTo: dateTo || null, filePath: filePath || null, allRefs
      });
      setHitCount(res.length);
      onFiltered(res);
    } catch (e: any) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  const handleReset = () => {
    setAuthor('');
    setDateFrom('');
    setDateTo('');
    setFilePath('');
    setAllRefs(false);
    setHitCount(null);
    setError('');
    onReset?.();
  };

  return (
    <motion.div className="analysis-panel" initial={{ opacity: 0, y: 12 }} animate={{ opacity: 1, y: 0 }}>
      <div className="panel-title-row">
        <h3>提交筛选</h3>
        {hitCount !== null && <span className="panel-hint">命中 {hitCount} 条</span>}
      </div>
      <form className="filter-form" onSubmit={applyFilter}>
        <div className="filter-grid">
          <label className="filter-field">
            <span>作者</span>
            <input className="filter-input" value={author} onChange={e => setAuthor(e.target.value)} placeholder="任意作者（模糊匹配）" />
          </label>
          <label className="filter-field">
            <span>文件路径</span>
            <input className="filter-input" value={filePath} onChange={e => setFilePath(e.target.value)} placeholder="如 src/main.rs" />
          </label>
          <label className="filter-field">
            <span>起始日期</span>
            <input className="filter-input" type="date" value={dateFrom} onChange={e => setDateFrom(e.target.value)} />
          </label>
          <label className="filter-field">
            <span>截止日期</span>
            <input className="filter-input" type="date" value={dateTo} onChange={e => setDateTo(e.target.value)} />
          </label>
        </div>
        <label className="filter-check">
          <input type="checkbox" checked={allRefs} onChange={e => setAllRefs(e.target.checked)} />
          <span>包含所有分支与标签</span>
        </label>
        <div className="filter-actions">
          <button className="btn btn-blue" type="submit" disabled={loading}>
            {loading ? '筛选中…' : '筛选'}
          </button>
          <button className="btn" type="button" onClick={handleReset} disabled={!hasCondition && hitCount === null}>
            重置
          </button>
        </div>
        {error && <div style={{ color: 'var(--danger)', fontSize: 12 }}>{error}</div>}
      </form>
    </motion.div>
  );
}
