import { AnimatePresence, motion } from 'framer-motion';

interface FeedbackOverlayProps {
  loading: boolean;
  loadingText?: string | null;
  toast?: string | null;
  onToastClose?: () => void;
}

export default function FeedbackOverlay({ loading, loadingText, toast, onToastClose }: FeedbackOverlayProps) {
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
