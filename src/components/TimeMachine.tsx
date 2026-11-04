import { useState, useEffect, useRef } from 'react';
import { motion } from 'framer-motion';
import { invokeTauri } from '../services/tauriService';

interface TimeMachineFile {
  path: string;
  size: number;
  is_directory: boolean;
}

interface TimeMachineSnapshot {
  commit_hash: string;
  author: string;
  time: string;
  message: string;
  files: TimeMachineFile[];
}

// 后端返回本地时区的 "YYYY-MM-DD HH:MM:SS"。空格格式在 WKWebView 里是
// Invalid Date，手动按本地时区解析，才能和提交的 unix 秒对上
function parseGitTime(s: string): number {
  const m = /^(\d{4})-(\d{2})-(\d{2})[ T](\d{2}):(\d{2}):(\d{2})$/.exec(s.trim());
  if (!m) return NaN;
  return new Date(+m[1], +m[2] - 1, +m[3], +m[4], +m[5], +m[6]).getTime() / 1000;
}

export default function TimeMachine({ repoPath }: { repoPath: string }) {
  const [snapshot, setSnapshot] = useState<TimeMachineSnapshot | null>(null);
  const [fileContent, setFileContent] = useState('');
  const [selectedFile, setSelectedFile] = useState('');
  // 游标是提交序号而不是时间：提交间隔可能几个月，按时间映射会让进度条大跳
  const [timestamps, setTimestamps] = useState<number[]>([]);
  const [currentIndex, setCurrentIndex] = useState(0);
  const [isPlaying, setIsPlaying] = useState(false);
  const [playSpeed, setPlaySpeed] = useState(1);
  const playTimerRef = useRef<number | null>(null);
  const timestampsRef = useRef<number[]>([]);
  const playIdxRef = useRef(0);
  // 上一张快照还没回来时跳过本次加载，避免请求堆积
  const playPendingRef = useRef(false);
  // 快照请求序号：晚到的旧响应不能覆盖新快照
  const snapshotSeqRef = useRef(0);
  // 播放中调速：定时器里读 ref，避免闭包拿到旧值
  const playSpeedRef = useRef(1);
  useEffect(() => { playSpeedRef.current = playSpeed; }, [playSpeed]);

  useEffect(() => {
    const loadTimeRange = async () => {
      try {
        const commits: any[] = await invokeTauri('get_commits', { path: repoPath });
        if (commits.length > 0) {
          const times = commits.map((c: any) => parseGitTime(c.time)).filter(t => !isNaN(t));
          if (times.length > 0) {
            const sorted = [...new Set(times)].sort((a, b) => a - b);
            timestampsRef.current = sorted;
            setTimestamps(sorted);
            setCurrentIndex(sorted.length - 1);
            loadSnapshot(sorted[sorted.length - 1]);
          }
        }
      } catch (e) { console.error(e); }
    };
    loadTimeRange();
  }, [repoPath]);

  // 卸载时停掉定时器
  useEffect(() => {
    return () => {
      if (playTimerRef.current !== null) window.clearTimeout(playTimerRef.current);
    };
  }, []);

  const loadSnapshot = async (ts: number) => {
    const seq = ++snapshotSeqRef.current;
    try {
      const res: TimeMachineSnapshot = await invokeTauri('get_time_machine_snapshot', { path: repoPath, timestamp: ts });
      if (seq === snapshotSeqRef.current) {
        setSnapshot(res);
      }
    } catch (e) { console.error(e); }
  };

  const handleSliderChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    if (isPlaying) stopPlay();
    const index = Number(e.target.value);
    setCurrentIndex(index);
    const ts = timestampsRef.current[index];
    if (ts !== undefined) loadSnapshot(ts);
  };

  const playInterval = (speed: number) => (speed >= 20 ? 60 : speed >= 5 ? 140 : 350);

  const startPlay = () => {
    const list = timestampsRef.current;
    if (isPlaying || list.length < 2) return;
    // 从下一条提交开始；已经在最后就从头播
    let idx = currentIndex + 1;
    if (idx >= list.length) idx = 0;
    playIdxRef.current = idx;
    setIsPlaying(true);

    const tick = () => {
      const times = timestampsRef.current;
      if (playIdxRef.current >= times.length) {
        setIsPlaying(false);
        playTimerRef.current = null;
        return;
      }
      const index = playIdxRef.current++;
      setCurrentIndex(index);
      if (!playPendingRef.current) {
        playPendingRef.current = true;
        void loadSnapshot(times[index]).finally(() => { playPendingRef.current = false; });
      }
      playTimerRef.current = window.setTimeout(tick, playInterval(playSpeedRef.current));
    };
    tick();
  };

  const stopPlay = () => {
    if (playTimerRef.current !== null) {
      window.clearTimeout(playTimerRef.current);
      playTimerRef.current = null;
    }
    setIsPlaying(false);
  };

  const loadFileContent = async (filePath: string) => {
    if (!snapshot) return;
    setSelectedFile(filePath);
    try {
      const content: string = await invokeTauri('get_file_content_at_commit', {
        path: repoPath,
        commitHash: snapshot.commit_hash,
        filePath,
      });
      setFileContent(content);
    } catch (e) { setFileContent('无法加载文件内容'); }
  };

  return (
    <motion.div className="analysis-panel" initial={{ opacity: 0, y: 12 }} animate={{ opacity: 1, y: 0 }}>
      <h3>时间机器</h3>

      <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginTop: 12 }}>
        <button className="btn" onClick={stopPlay} disabled={!isPlaying} style={{ padding: '4px 12px' }}>⏸</button>
        <button className="btn btn-blue" onClick={startPlay} disabled={isPlaying} style={{ padding: '4px 12px' }}>▶</button>
        <select value={playSpeed} onChange={e => setPlaySpeed(Number(e.target.value))} style={{ padding: '4px 8px' }}>
          <option value={1}>1x</option>
          <option value={5}>5x</option>
          <option value={20}>20x</option>
        </select>
        <span style={{ fontSize: 13, color: 'var(--text-dim)', marginLeft: 'auto' }}>
          {snapshot?.time || '加载中...'}
          {timestamps.length > 1 && ` · ${currentIndex + 1}/${timestamps.length}`}
        </span>
      </div>

      <input
        type="range"
        min={0}
        max={Math.max(timestamps.length - 1, 0)}
        value={currentIndex}
        onChange={handleSliderChange}
        disabled={timestamps.length < 2}
        style={{ width: '100%', marginTop: 12 }}
      />

      {snapshot && (
        <div style={{ marginTop: 12 }}>
          <div className="analysis-item">
            <span className="hash">{snapshot.commit_hash.substring(0, 8)}</span>
            <span className="author">{snapshot.author}</span>
            <span className="message" style={{ marginLeft: 8 }}>{snapshot.message.split('\n')[0]}</span>
          </div>

          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12, marginTop: 12 }}>
            <div>
              <div className="section-title">文件树</div>
              <div style={{ maxHeight: 300, overflowY: 'auto' }}>
                {snapshot.files.map(f => (
                  <div
                    key={f.path}
                    className="analysis-item"
                    onClick={() => !f.is_directory && loadFileContent(f.path)}
                    style={{ cursor: f.is_directory ? 'default' : 'pointer', display: 'flex', justifyContent: 'space-between' }}
                  >
                    <span>{f.is_directory ? '📁' : '📄'} {f.path}</span>
                    {!f.is_directory && <span style={{ fontSize: 11, color: 'var(--text-dim)' }}>{(f.size / 1024).toFixed(1)} KB</span>}
                  </div>
                ))}
              </div>
            </div>
            <div>
              <div className="section-title">代码预览</div>
              <pre style={{
                background: 'var(--code-bg)',
                border: '1px solid var(--border)',
                borderRadius: 8,
                padding: 12,
                maxHeight: 300,
                overflow: 'auto',
                fontSize: 12,
                whiteSpace: 'pre-wrap',
                color: 'var(--text)',
                fontFamily: 'var(--font-mono)',
              }}>
                {fileContent || (selectedFile ? '加载中...' : '点击文件查看代码')}
              </pre>
            </div>
          </div>
        </div>
      )}
    </motion.div>
  );
}
