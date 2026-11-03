import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { motion } from 'framer-motion';

interface DirectoryKnowledge {
  path: string;
  total_lines: number;
  authors: number;
  top_author: string;
  top_share: number;
  bus_factor: number;
}

export default function KnowledgeReport({ repoPath }: { repoPath: string }) {
  const [rows, setRows] = useState<DirectoryKnowledge[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError('');
    invoke<DirectoryKnowledge[]>('get_knowledge_report', { path: repoPath })
      .then(res => { if (!cancelled) setRows(res); })
      .catch(e => { if (!cancelled) setError(String(e)); })
      .finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [repoPath]);

  return (
    <motion.div className="analysis-panel" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }}>
      <h3>知识集中度</h3>
      <div style={{ fontSize: 12, color: 'var(--text-dim)', marginBottom: 8 }}>
        巴士因子 = 覆盖 50% 改动需要的人数，1 表示单点依赖
      </div>
      {error && <div className="analysis-item" style={{ color: 'var(--danger)' }}>{error}</div>}
      {loading && rows.length === 0 && (
        <div className="analysis-item" style={{ color: 'var(--text-dim)' }}>分析中...</div>
      )}
      {rows.map(r => (
        <div key={r.path} className="analysis-item" style={{ gap: 10 }}>
          <span className="file-path" style={{ minWidth: 110 }}>{r.path}</span>
          <div style={{ flex: 1, height: 6, background: 'var(--panel-strong)', borderRadius: 3, minWidth: 60 }}>
            <div style={{ width: `${Math.round(r.top_share * 100)}%`, height: '100%', background: 'var(--accent)', borderRadius: 3 }} />
          </div>
          <span style={{ fontSize: 12, color: 'var(--text-secondary)', whiteSpace: 'nowrap' }}>
            {r.top_author} {Math.round(r.top_share * 100)}%
          </span>
          <span style={{ fontSize: 11, color: 'var(--text-dim)', whiteSpace: 'nowrap' }}>
            {r.authors} 人 · {r.total_lines} 行
          </span>
          <span className={`bus-badge bus-${Math.min(r.bus_factor, 3)}`}>巴士 {r.bus_factor}</span>
        </div>
      ))}
      {!loading && rows.length === 0 && !error && <div className="analysis-item">没有可分析的数据</div>}
    </motion.div>
  );
}
