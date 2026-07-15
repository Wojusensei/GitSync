import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { motion } from 'framer-motion';
import { VscFolder, VscFolderOpened } from 'react-icons/vsc';
import { getFileIcon } from '../utils/fileIcons';

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

    const sortedChildren = node.children.sort((a, b) => {
      if (a.is_directory !== b.is_directory) {
        return b.is_directory ? 1 : -1; // 文件夹排在前面
      }
      return a.name.localeCompare(b.name); // 同类型按名字排序
    });
    return (
      <div key={fullPath} style={{ paddingLeft: parentPath ? 16 : 0 }}>
        <div
          className="analysis-item"
          onClick={() => { node.is_directory ? toggleExpand(fullPath) : onSelectFile(fullPath); }}
          style={{ cursor: 'pointer', display: 'flex', justifyContent: 'flex-start', alignItems: 'center', padding: '6px 0' }}
        >
          {node.is_directory ? (
            expanded.has(fullPath) ? (
              <VscFolderOpened size={16} style={{ color: '#ffca28', marginRight: 8, flexShrink: 0 }} />
            ) : (
              <VscFolder size={16} style={{ color: '#ffca28', marginRight: 8, flexShrink: 0 }} />
            )
          ) : (
            <span style={{ marginRight: 8, display: 'inline-flex', alignItems: 'center', flexShrink: 0 }}>
              {getFileIcon(node.name, 16)}
            </span>
          )}
          <span style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{node.name}</span>
        </div>
        {node.is_directory && expanded.has(fullPath) && sortedChildren.map(c => renderNode(c, fullPath))}
      </div>
    );
  };

  return (
    <motion.div className="analysis-panel" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }}>
      <h3>文件树</h3>
      <button className="btn btn-blue" onClick={loadTree}>加载文件树</button>
      <div style={{ maxHeight: 400, overflowY: 'auto', marginTop: 12 }}>
        {nodes.sort((a, b) => {
          if (a.is_directory !== b.is_directory) {
            return b.is_directory ? 1 : -1; // 文件夹排在前面
          }
          return a.name.localeCompare(b.name); // 同类型按名字排序
        }).map(n => renderNode(n))}
      </div>
    </motion.div>
  );
}