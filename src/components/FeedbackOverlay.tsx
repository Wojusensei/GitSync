import { useEffect, useRef } from 'react';
import { AnimatePresence, motion } from 'framer-motion';

interface FeedbackOverlayProps {
  loading: boolean;
  loadingText?: string | null;
  toast?: string | null;
  onToastClose?: () => void;
}

export default function FeedbackOverlay({ loading, loadingText, toast, onToastClose }: FeedbackOverlayProps) {
  // 回调每次渲染都是新函数，用 ref 持有，避免定时器被重置
  const closeRef = useRef(onToastClose);
  closeRef.current = onToastClose;

  // 提示 2.6 秒后自动消失
  useEffect(() => {
    if (!toast) return;
    const timer = window.setTimeout(() => closeRef.current?.(), 2600);
    return () => window.clearTimeout(timer);
  }, [toast]);

  return (
    <>
      <AnimatePresence>
        {loading && (
          <motion.div
            className="feedback-loading-overlay"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
          >
            <div className="feedback-loading-card">
              <div className="feedback-spinner" />
              <div className="feedback-loading-text">{loadingText || '处理中...'}</div>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      <AnimatePresence>
        {toast && !loading && (
          <motion.div
            className="feedback-toast"
            initial={{ opacity: 0, y: 16 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 16 }}
            transition={{ duration: 0.2 }}
          >
            <span>{toast}</span>
            {onToastClose && (
              <button className="feedback-toast-close" onClick={onToastClose} aria-label="关闭提示">×</button>
            )}
          </motion.div>
        )}
      </AnimatePresence>
    </>
  );
}
