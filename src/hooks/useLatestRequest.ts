import { useCallback, useRef } from 'react';

// 防竞态：只有最后一次发起的请求允许写 state
export function useLatestRequest() {
  const seqRef = useRef(0);
  return useCallback(() => {
    const seq = ++seqRef.current;
    return () => seq === seqRef.current;
  }, []);
}
