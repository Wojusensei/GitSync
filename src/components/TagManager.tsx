import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { motion } from 'framer-motion';

interface TagInfo {
  name: string;
  commit_hash: string;
}

export default function TagManager({ repoPath }: { repoPath: string }) {
  const [tags, setTags] = useState<TagInfo[]>([]);
  const [newName, setNewName] = useState('');
  const [newHash, setNewHash] = useState('');

  const loadTags = async () => {
    setTags(await invoke<TagInfo[]>('get_tags', { path: repoPath }));
  };

  const createTag = async () => {
    await invoke('create_tag', { path: repoPath, name: newName, commitHash: newHash });
    loadTags();
    setNewName('');
    setNewHash('');
  };

  return (
    <motion.div className="analysis-panel" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }}>
      <h3>标签管理</h3>
      <button className="btn btn-blue" onClick={loadTags}>加载标签</button>
      <div style={{ display: 'flex', gap: 8, marginTop: 12 }}>
        <input className="path-input" value={newName} onChange={e => setNewName(e.target.value)} placeholder="标签名" />
        <input className="path-input" value={newHash} onChange={e => setNewHash(e.target.value)} placeholder="提交哈希" />
        <button className="btn btn-blue" onClick={createTag}>创建</button>
      </div>
      {tags.map(t => (
        <div key={t.name} className="analysis-item">
          <span className="hash">{t.name}</span>
          <span className="message">{t.commit_hash.substring(0, 8)}</span>
        </div>
      ))}
    </motion.div>
  );
}