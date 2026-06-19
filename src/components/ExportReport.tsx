import { motion } from 'framer-motion';

export default function ExportReport({ healthReport, contributors, hotFiles }: any) {
  const exportMarkdown = () => {
    let md = "# 仓库分析报告\n\n";
    if (healthReport) {
      md += "## 健康报告\n";
      md += `- 大文件: ${healthReport.large_files.join(", ")}\n`;
      md += `- 无上游分支: ${healthReport.stale_branches.join(", ")}\n`;
      md += `- 冲突文件: ${healthReport.conflicts.join(", ")}\n\n`;
    }
    if (contributors.length) {
      md += "## 贡献者统计\n";
      contributors.forEach((c: any) => md += `- ${c.author}: ${c.commits} 次提交, +${c.additions}/-${c.deletions}\n`);
    }
    if (hotFiles.length) {
      md += "\n## 热点文件\n";
      hotFiles.forEach((f: any) => md += `- ${f.path}: ${f.changes} 次变更\n`);
    }
    navigator.clipboard.writeText(md);
    alert('Markdown 报告已复制到剪贴板');
  };

  return (
    <motion.div className="analysis-panel" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: 'auto' }}>
      <h3>导出报告</h3>
      <button className="btn btn-blue" onClick={exportMarkdown}>导出 Markdown 到剪贴板</button>
    </motion.div>
  );
}