import { useRef, useState } from 'react';

export interface ScrubberFile {
  path: string;
  status: string;
}

export default function FileScrubber({ files, index, onChange }: {
  files: ScrubberFile[];
  index: number;
  onChange: (index: number) => void;
}) {
  const trackRef = useRef<HTMLDivElement>(null);
  const [dragging, setDragging] = useState(false);
  // 拖动中滑块跟手自由移动，松手后才吸附到最近的文件节点
  const [dragRatio, setDragRatio] = useState<number | null>(null);
  const count = files.length;
  const ratio = dragRatio ?? (count > 1 ? index / (count - 1) : 0);

  const ratioFromX = (clientX: number) => {
    const track = trackRef.current;
    if (!track) return null;
    const rect = track.getBoundingClientRect();
    return Math.min(1, Math.max(0, (clientX - rect.left) / rect.width));
  };

  const snap = (value: number) => {
    if (count < 2) return;
    onChange(Math.round(value * (count - 1)));
  };

  return (
    <div
      ref={trackRef}
      className={`file-scrubber${dragging ? ' dragging' : ''}`}
      onPointerDown={(e) => {
        e.preventDefault();
        e.currentTarget.setPointerCapture(e.pointerId);
        setDragging(true);
        const r = ratioFromX(e.clientX);
        if (r !== null) setDragRatio(r);
      }}
      onPointerMove={(e) => {
        if (!dragging) return;
        const r = ratioFromX(e.clientX);
        if (r !== null) setDragRatio(r);
      }}
      onPointerUp={(e) => {
        const r = dragRatio ?? ratioFromX(e.clientX);
        if (r !== null) snap(r);
        setDragging(false);
        setDragRatio(null);
        if (e.currentTarget.hasPointerCapture(e.pointerId)) e.currentTarget.releasePointerCapture(e.pointerId);
      }}
      onPointerCancel={() => {
        setDragging(false);
        setDragRatio(null);
      }}
      role="slider"
      aria-label="文件变更进度"
      aria-valuemin={1}
      aria-valuemax={count}
      aria-valuenow={index + 1}
    >
      <div className="file-scrubber-rail" />
      <div className="file-scrubber-fill" style={{ width: `${ratio * 100}%` }} />
      {files.map((file, i) => (
        <span
          key={`${file.path}-${i}`}
          className={`file-scrubber-dot${i === index ? ' active' : ''}`}
          style={{ left: count > 1 ? `${(i / (count - 1)) * 100}%` : '0%' }}
          title={`${i + 1}. ${file.path}`}
        />
      ))}
      <span className="file-scrubber-thumb" style={{ left: `${ratio * 100}%` }} />
    </div>
  );
}
