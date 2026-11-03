import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { motion } from 'framer-motion';

interface SearchResult {
  commit_hash: string;
  author: string;
  time: string;
  file_path: string;
  line_number: number;
  content: string;
}

export default function SemanticSearch({ repoPath }: { repoPath: string }) {
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<SearchResult[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');
  const [allRefs, setAllRefs] = useState(false);

  const handleSearch = async () => {
    setLoading(true);
    setError('');
    try {
      const res = await invoke<SearchResult[]>('semantic_search', { path: repoPath, query, allRefs });
      setResults(res);
    } catch (e: any) {
      setError(String(e));
    } finally { setLoading(false); }
  };

  return (
    <motion.div className="analysis-panel" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }}>
      <h3>语义代码搜索</h3>
      <div style={{ display: 'flex', gap: 8, marginBottom: 8, flexWrap: 'wrap' }}>
        <input className="path-input" value={query} onChange={e => setQuery(e.target.value)} placeholder="搜索代码内容..." style={{ flex: 1 }} />
        <button className="btn btn-blue" onClick={handleSearch} disabled={loading}>搜索</button>
      </div>
      <label style={{ display: 'flex', alignItems: 'center', gap: 4, fontSize: 12, color: 'var(--text-dim)', marginBottom: 12, cursor: 'pointer' }}>
        <input type="checkbox" checked={allRefs} onChange={e => setAllRefs(e.target.checked)} />
        包含所有分支与标签
      </label>
      {error && <div className="analysis-item" style={{ color: 'var(--danger)' }}>{error}</div>}
      {results.length === 0 && !loading && !error && (
        <div className="analysis-item" style={{ color: 'var(--text-dim)' }}>没有匹配结果，输入内容后点击搜索</div>
      )}
      {results.map((r, i) => (
        <div key={i} className="analysis-item">
          <span className="hash">{r.commit_hash.substring(0, 8)}</span>
          <span className="file-path">{r.file_path}:{r.line_number}</span>
          <span className="message">{r.content}</span>
        </div>
      ))}
    </motion.div>
  );
}
