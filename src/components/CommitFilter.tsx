import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { motion } from 'framer-motion';

export default function CommitFilter({ repoPath, onFiltered }: { repoPath: string; onFiltered: (commits: any[]) => void }) {
  const [author, setAuthor] = useState('');
  const [dateFrom, setDateFrom] = useState('');
  const [dateTo, setDateTo] = useState('');
  const [filePath, setFilePath] = useState('');

  const applyFilter = async () => {
    const res = await invoke<any[]>('filter_commits', {
      path: repoPath, author: author || null, dateFrom: dateFrom || null, dateTo: dateTo || null, filePath: filePath || null
    });
    onFiltered(res);
  };

  return (
    <motion.div className="analysis-panel" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }}>
      <h3>提交筛选</h3>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
        <input className="path-input" value={author} onChange={e => setAuthor(e.target.value)} placeholder="作者" />
        <input className="path-input" value={dateFrom} onChange={e => setDateFrom(e.target.value)} placeholder="起始日期 (YYYY-MM-DD)" />
        <input className="path-input" value={dateTo} onChange={e => setDateTo(e.target.value)} placeholder="截止日期 (YYYY-MM-DD)" />
        <input className="path-input" value={filePath} onChange={e => setFilePath(e.target.value)} placeholder="文件路径" />
        <button className="btn btn-blue" onClick={applyFilter}>筛选</button>
      </div>
    </motion.div>
  );
}