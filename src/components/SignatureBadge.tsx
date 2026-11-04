import { useEffect, useState } from 'react';
import { invokeTauri } from '../services/tauriService';

interface SignatureInfo {
  signed: boolean;
  kind: string | null;
}

export default function SignatureBadge({ repoPath, commitHash }: { repoPath: string; commitHash: string }) {
  const [info, setInfo] = useState<SignatureInfo | null>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const res = await invokeTauri<SignatureInfo>('get_commit_signature', { path: repoPath, commitHash });
        if (!cancelled) setInfo(res);
      } catch {
        // 签名读取失败不影响详情展示
      }
    })();
    return () => { cancelled = true; };
  }, [repoPath, commitHash]);

  if (!info) return null;
  if (!info.signed) {
    return <span className="sig-badge unsigned" title="提交未签名">未签名</span>;
  }
  return (
    <span className="sig-badge signed" title={`提交带有 ${info.kind} 签名`}>
      已签名{info.kind ? `（${info.kind}）` : ''}
    </span>
  );
}
