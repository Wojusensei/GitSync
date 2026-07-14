import { VscWarning } from 'react-icons/vsc';
import GlassModal from './GlassModal';

interface SafeDirectoryModalProps {
  path: string | null;
  fixing: boolean;
  error: string | null;
  onClose: () => void;
  onFix: () => void;
}

export default function SafeDirectoryModal({ path, fixing, error, onClose, onFix }: SafeDirectoryModalProps) {
  if (!path) return null;

  return (
    <GlassModal
      isOpen={Boolean(path)}
      title="检测到 Git 安全目录错误"
      onClose={onClose}
      footer={(
        <>
          <button className="btn" onClick={onClose} disabled={fixing}>取消</button>
          <button className="btn btn-blue" onClick={onFix} disabled={fixing}>{fixing ? '正在修复...' : '执行修复'}</button>
        </>
      )}
    >
      <div className="safe-dir-body">
        <div className="safe-dir-title">
          <VscWarning size={24} />
          <span>由于 Git 安全策略，该仓库路径不属于当前用户。</span>
        </div>
        <p>此问题通常发生在跨用户、跨系统或目录权限变更后。修复后会自动重新加载仓库。</p>
        <div className="safe-dir-path-box">{path}</div>
        <p>执行以下命令以注册安全目录：</p>
        <div className="safe-dir-cmd-box">git config --global --add safe.directory &quot;{path}&quot;</div>
        {error && <div className="safe-dir-error">修复失败: {error}</div>}
      </div>
    </GlassModal>
  );
}
