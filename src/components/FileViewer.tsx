import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { motion } from 'framer-motion';
import { VscClose } from 'react-icons/vsc';
import { getFileIcon } from '../utils/fileIcons';
import { renderHighlightedLine } from '../utils/diffUtils';

interface FileViewerProps {
  repoPath: string;
  filePath: string;
  onClose: () => void;
}

export default function FileViewer({ repoPath, filePath, onClose }: FileViewerProps) {
  const [content, setContent] = useState<string>('');
  const [loading, setLoading] = useState<boolean>(false);
  const [error, setError] = useState<string>('');

  useEffect(() => {
    const fetchFileContent = async () => {
      setLoading(true);
      setError('');
      try {
        const res = await invoke<string>('get_file_content', { path: repoPath, filePath });
        setContent(res);
      } catch (e: any) {
        setError(String(e));
      } finally {
        setLoading(false);
      }
    };
    fetchFileContent();
  }, [repoPath, filePath]);

  const lines = content.split('\n');

  // Check if it's binary content
  const isBinary = content.includes('\u0000') || (content.length > 0 && content.slice(0, 1000).replace(/[\x09\x0A\x0D\x20-\x7E]/g, '').length / Math.min(1000, content.length) > 0.3);

  return (
    <motion.div
      className="analysis-panel"
      initial={{ opacity: 0, x: 20 }}
      animate={{ opacity: 1, x: 0 }}
      style={{ display: 'flex', flexDirection: 'column', height: '100%', minHeight: 400 }}
    >
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', borderBottom: '1px solid rgba(255,255,255,0.08)', paddingBottom: 10, marginBottom: 12 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, overflow: 'hidden' }}>
          {getFileIcon(filePath, 18)}
          <span style={{ fontSize: 14, fontWeight: 600, color: '#dfe6e9', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
            {filePath}
          </span>
        </div>
        <button
          onClick={onClose}
          style={{ background: 'transparent', border: 'none', color: '#c8d6e5', cursor: 'pointer', display: 'flex', alignItems: 'center', padding: 4 }}
          title="关闭查看"
        >
          <VscClose size={18} />
        </button>
      </div>

      {loading ? (
        <div style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center', color: 'rgba(255,255,255,0.5)', minHeight: 300 }}>
          加载文件中...
        </div>
      ) : error ? (
        <div style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center', color: '#ff6b6b', minHeight: 300 }}>
          {error}
        </div>
      ) : isBinary ? (
        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', color: 'rgba(255,255,255,0.4)', minHeight: 300 }}>
          <p>此文件可能是二进制文件，无法直接预览。</p>
        </div>
      ) : (
        <div
          style={{
            flex: 1,
            overflow: 'auto',
            maxHeight: 500,
            background: 'rgba(0,0,0,0.2)',
            borderRadius: 8,
            border: '1px solid rgba(255,255,255,0.05)',
            fontFamily: 'Consolas, Monaco, "Courier New", monospace',
            fontSize: 12,
            lineHeight: 1.5,
            padding: '10px 0'
          }}
        >
          <table style={{ borderCollapse: 'collapse', width: '100%' }}>
            <tbody>
              {lines.map((line, idx) => (
                <tr key={idx} className="file-view-tr" style={{ verticalAlign: 'top' }}>
                  <td
                    style={{
                      width: 40,
                      textAlign: 'right',
                      paddingRight: 10,
                      userSelect: 'none',
                      color: 'rgba(255,255,255,0.3)',
                      borderRight: '1px solid rgba(255,255,255,0.05)',
                      fontSize: 11
                    }}
                  >
                    {idx + 1}
                  </td>
                  <td style={{ paddingLeft: 10, whiteSpace: 'pre', color: '#c8d6e5' }}>
                    {renderHighlightedLine(line)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </motion.div>
  );
}
