import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { motion } from 'framer-motion';

interface RemoteInfo {
  name: string;
  url: string;
}

export default function RemoteManager({ repoPath }: { repoPath: string }) {
  const [remotes, setRemotes] = useState<RemoteInfo[]>([]);

  const loadRemotes = async () => {
    setRemotes(await invoke<RemoteInfo[]>('get_remotes', { path: repoPath }));
  };

  return (
    <motion.div className="analysis-panel" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }}>
      <h3>远程仓库</h3>
      <button className="btn btn-blue" onClick={loadRemotes}>加载远程</button>
      {remotes.map(r => (
        <div key={r.name} className="analysis-item">
          <span className="hash">{r.name}</span>
          <span className="message">{r.url}</span>
        </div>
      ))}
    </motion.div>
  );
}