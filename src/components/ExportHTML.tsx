import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { motion } from 'framer-motion';

// 报告内容来自仓库的提交信息/作者名/文件路径（可能是克隆来的恶意仓库），
// 直接拼进 HTML 会在浏览器打开报告时执行任意脚本
const escapeHtml = (s: string) =>
  s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');

export default function ExportHTML({ repoPath }: { repoPath: string }) {
  const [html, setHtml] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');

  const exportReport = async () => {
    setLoading(true);
    setError('');
    try {
      const md = await invoke<string>('export_report_markdown', { path: repoPath });
      const fullHtml = `<html><head><meta charset="utf-8"><title>仓库分析报告</title></head><body style="font-family: sans-serif; max-width: 800px; margin: 40px auto; padding: 20px;"><pre style="white-space: pre-wrap; line-height: 1.6;">${escapeHtml(md)}</pre></body></html>`;
      setHtml(fullHtml);
      await navigator.clipboard.writeText(fullHtml);
      alert('HTML 报告已复制到剪贴板');
    } catch (e: any) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  return (
    <motion.div className="analysis-panel" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }}>
      <h3>导出报告</h3>
      <button className="btn btn-blue" onClick={exportReport} disabled={loading}>
        {loading ? '生成中...' : '生成并复制 HTML'}
      </button>
      {error && <div className="analysis-item" style={{ color: '#ff6b6b' }}>{error}</div>}
      {html && (
        <div style={{ marginTop: 12 }}>
          <div className="section-title">预览</div>
          <div style={{ maxHeight: 400, overflow: 'auto', background: 'rgba(255,255,255,0.03)', padding: 12, borderRadius: 8, fontSize: 12, whiteSpace: 'pre-wrap' }}>
            {html}
          </div>
        </div>
      )}
    </motion.div>
  );
}