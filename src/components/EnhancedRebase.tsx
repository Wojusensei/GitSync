import { useState } from 'react';
import { motion, Reorder } from 'framer-motion';
import { invokeTauri } from '../services/tauriService';

interface RebaseCommit {
  hash: string;
  message: string;
  author: string;
  time: string;
}

interface RebaseOperation {
  hash: string;
  action: string;
  new_message?: string;
}

export default function EnhancedRebase({ repoPath, onComplete }: { repoPath: string; onComplete: () => void }) {
  
  const [ops, setOps] = useState<RebaseOperation[]>([]);
  const [loading, setLoading] = useState(false);

  const load = async () => {
    try {
      const res = await invokeTauri<RebaseCommit[]>('get_rebase_commits', { path: repoPath, count: 20 });
      setOps(res.map(c => ({ hash: c.hash, action: 'pick' })));
    } catch (e) { console.error(e); }
  };

  const execute = async () => {
    setLoading(true);
    try {
      await invokeTauri('execute_rebase', { path: repoPath, operations: ops });
      onComplete();
    } catch (e) { console.error(e); }
    finally { setLoading(false); }
  };

  return (
    <motion.div className="analysis-panel" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }}>
      <h3>交互 Rebase 增强</h3>
      <button className="btn btn-blue" onClick={load} style={{ marginBottom: 12 }}>加载提交</button>
      <Reorder.Group values={ops} onReorder={setOps}>
        {ops.map((op, idx) => (
          <Reorder.Item key={op.hash} value={op}>
            <div className="rebase-item">
              <span className="hash">{op.hash.substring(0, 8)}</span>
              <select value={op.action} onChange={(e) => { const newOps = [...ops]; newOps[idx].action = e.target.value; setOps(newOps); }}>
                <option value="pick">pick</option>
                <option value="squash">squash</option>
                <option value="drop">drop</option>
                <option value="reword">reword</option>
              </select>
            </div>
          </Reorder.Item>
        ))}
      </Reorder.Group>
      <button className="btn btn-blue" onClick={execute} disabled={loading} style={{ marginTop: 12 }}>执行 Rebase</button>
    </motion.div>
  );
}