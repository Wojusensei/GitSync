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

  const handleSearch = async () => {
    setLoading(true);
    try {
      const res = await invoke<SearchResult[]>('semantic_search', { path: repoPath, query });
      setResults(res);
    } catch (e) { console.error(e); }
    finally { setLoading(false); }
  };

  return (
    <motion.div className="analysis-panel" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }}>
      <h3>语义代码搜索</h3>
      <div style={{ display: 'flex', gap: 8, marginBottom: 12 }}>
        <input className="path-input" value={query} onChange={e => setQuery(e.target.value)} placeholder="搜索代码内容..." style={{ flex: 1 }} />
        <button className="btn btn-blue" onClick={handleSearch} disabled={loading}>搜索</button>
      </div>
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