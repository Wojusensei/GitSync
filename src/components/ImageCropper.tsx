import { useEffect, useRef, useState } from 'react';

const VIEW_W = 520;

// 按窗口比例框选背景图：拖动平移、滚轮/滑杆缩放，确认后导出与屏幕同比例的图片
export default function ImageCropper({ src, onConfirm, onCancel }: {
  src: string;
  onConfirm: (croppedBase64: string) => void;
  onCancel: () => void;
}) {
  const imgRef = useRef<HTMLImageElement>(null);
  const viewportRef = useRef<HTMLDivElement>(null);
  const dragRef = useRef<{ x: number; y: number; ox: number; oy: number } | null>(null);
  const [ready, setReady] = useState(false);
  const [zoom, setZoom] = useState(1);
  const [offset, setOffset] = useState({ x: 0, y: 0 });

  const aspect = window.innerWidth / window.innerHeight;
  const viewW = VIEW_W;
  const viewH = Math.round(VIEW_W / aspect);

  const iw = imgRef.current?.naturalWidth || 1;
  const ih = imgRef.current?.naturalHeight || 1;
  const scale = Math.max(viewW / iw, viewH / ih) * zoom;
  const dw = iw * scale;
  const dh = ih * scale;

  const clamp = (x: number, y: number) => ({
    x: Math.min(0, Math.max(viewW - dw, x)),
    y: Math.min(0, Math.max(viewH - dh, y)),
  });

  const center = () => {
    const s = Math.max(viewW / iw, viewH / ih);
    setOffset({ x: (viewW - iw * s) / 2, y: (viewH - ih * s) / 2 });
  };

  const handleLoad = () => {
    setReady(true);
    center();
  };

  // 缩放后把画面拉回可视范围，避免露出底色
  useEffect(() => {
    setOffset(o => clamp(o.x, o.y));
  }, [zoom, ready]);

  useEffect(() => {
    const el = viewportRef.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      setZoom(z => Math.min(3, Math.max(1, z - e.deltaY * 0.001)));
    };
    el.addEventListener('wheel', onWheel, { passive: false });
    return () => el.removeEventListener('wheel', onWheel);
  }, []);

  const confirm = () => {
    const img = imgRef.current;
    if (!img) return;
    const outW = 1680;
    const outH = Math.round(outW / aspect);
    const canvas = document.createElement('canvas');
    canvas.width = outW;
    canvas.height = outH;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    ctx.drawImage(img, -offset.x / scale, -offset.y / scale, viewW / scale, viewH / scale, 0, 0, outW, outH);
    onConfirm(canvas.toDataURL('image/jpeg', 0.9).split(',')[1]);
  };

  return (
    <div className="cropper-overlay">
      <div className="cropper-card">
        <div className="panel-title-row" style={{ marginBottom: 10 }}>
          <h3>调整背景画面</h3>
          <span className="panel-hint">拖动调整位置，滚轮缩放</span>
        </div>
        <div
          ref={viewportRef}
          className="cropper-viewport"
          style={{ width: viewW, height: viewH }}
          onPointerDown={(e) => {
            e.preventDefault();
            e.currentTarget.setPointerCapture(e.pointerId);
            dragRef.current = { x: e.clientX, y: e.clientY, ox: offset.x, oy: offset.y };
          }}
          onPointerMove={(e) => {
            const drag = dragRef.current;
            if (!drag) return;
            setOffset(clamp(drag.ox + e.clientX - drag.x, drag.oy + e.clientY - drag.y));
          }}
          onPointerUp={(e) => {
            dragRef.current = null;
            if (e.currentTarget.hasPointerCapture(e.pointerId)) e.currentTarget.releasePointerCapture(e.pointerId);
          }}
          onPointerCancel={() => { dragRef.current = null; }}
        >
          <img
            ref={imgRef}
            src={`data:image/jpeg;base64,${src}`}
            alt="待裁剪背景"
            draggable={false}
            onLoad={handleLoad}
            style={{
              position: 'absolute',
              left: offset.x,
              top: offset.y,
              width: dw,
              height: dh,
              maxWidth: 'none',
              userSelect: 'none',
              pointerEvents: 'none',
            }}
          />
        </div>
        <div className="cropper-toolbar">
          <span style={{ fontSize: 12, color: 'var(--text-dim)', flexShrink: 0 }}>缩放</span>
          <input type="range" min="1" max="3" step="0.01" value={zoom} onChange={e => setZoom(Number(e.target.value))} />
          <button className="btn" type="button" onClick={center} style={{ padding: '4px 12px', fontSize: 12 }}>重置</button>
        </div>
        <div className="cropper-toolbar" style={{ justifyContent: 'flex-end', marginTop: 8 }}>
          <button className="btn" type="button" onClick={onCancel}>取消</button>
          <button className="btn btn-blue" type="button" onClick={confirm} disabled={!ready}>使用此画面</button>
        </div>
      </div>
    </div>
  );
}
