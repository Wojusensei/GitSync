import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { motion } from 'framer-motion';

interface TreeNode {
  name: string;
  is_directory: boolean;
  children: TreeNode[];
}

export default function FileTree({ repoPath, onSelectFile }: { repoPath: string; onSelectFile: (path: string) => void }) {
  const [nodes, setNodes] = useState<TreeNode[]>([]);
  const [expanded, setExpanded] = useState<Set<string>>(new Set());

  const loadTree = async () => {
    const res = await invoke<TreeNode[]>('get_file_tree', { path: repoPath });
    setNodes(res);
  };

  const toggleExpand = (path: string) => {
    const newSet = new Set(expanded);
    newSet.has(path) ? newSet.delete(path) : newSet.add(path);
    setExpanded(newSet);
  };

  const renderNode = (node: TreeNode, parentPath = '') => {
    const fullPath = parentPath ? `${parentPath}/${node.name}` : node.name;
    return (
      <div key={fullPath} style={{ paddingLeft: parentPath ? 16 : 0 }}>
        <div
          className="analysis-item"
          onClick={() => { node.is_directory ? toggleExpand(fullPath) : onSelectFile(fullPath); }}
          style={{ cursor: 'pointer' }}
        >
          {node.is_directory ? (expanded.has(fullPath) ? '📂 ' : '📁 ') : '📄 '}
          {node.name}
        </div>
        {node.is_directory && expanded.has(fullPath) && node.children.map(c => renderNode(c, fullPath))}
      </div>
    );
  };

  return (
    <motion.div className="analysis-panel" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }}>
      <h3>文件树</h3>
      <button className="btn btn-blue" onClick={loadTree}>加载文件树</button>
      <div style={{ maxHeight: 400, overflowY: 'auto', marginTop: 12 }}>
        {nodes.map(n => renderNode(n))}
      </div>
    </motion.div>
  );
}