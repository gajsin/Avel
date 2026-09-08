import { useState, useRef, useEffect, useCallback } from 'react';

export function useCopyFeedback<T = number | string>(timeoutMs: number = 1200) {
  const [copiedValue, setCopiedValue] = useState<T | null>(null);
  const timeoutRef = useRef<number | null>(null);

  useEffect(() => {
    return () => {
      if (timeoutRef.current) {
        window.clearTimeout(timeoutRef.current);
      }
    };
  }, []);

  const triggerCopy = useCallback(
    (value: T) => {
      if (timeoutRef.current) {
        window.clearTimeout(timeoutRef.current);
      }
      setCopiedValue(value);
      timeoutRef.current = window.setTimeout(() => {
        setCopiedValue(null);
        timeoutRef.current = null;
      }, timeoutMs);
    },
    [timeoutMs]
  );

  return [copiedValue, triggerCopy] as const;
}
