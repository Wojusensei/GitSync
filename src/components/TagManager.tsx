import { useState } from 'react';
import { motion } from 'framer-motion';
import { invokeTauri } from '../services/tauriService';

interface TagInfo {
  name: string;
  commit_hash: string;
}

export default function TagManager({ repoPath }: { repoPath: string }) {
  const [tags, setTags] = useState<TagInfo[]>([]);
  const [newName, setNewName] = useState('');
  const [newHash, setNewHash] = useState('');
  const [error, setError] = useState('');

  const loadTags = async () => {
    setError('');
    try {
      setTags(await invokeTauri<TagInfo[]>('get_tags', { path: repoPath }));
    } catch (e: any) {
      setError(String(e));
    }
  };

  const createTag = async () => {
    setError('');
    try {
      await invokeTauri('create_tag', { path: repoPath, name: newName, commitHash: newHash });
      loadTags();
      setNewName('');
      setNewHash('');
    } catch (e: any) {
      setError(String(e));
    }
  };

  return (
    <motion.div className="analysis-panel" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }}>
      <h3>标签管理</h3>
      <button className="btn btn-blue" onClick={loadTags}>加载标签</button>
      {error && <div className="analysis-item" style={{ color: '#ff6b6b' }}>{error}</div>}
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