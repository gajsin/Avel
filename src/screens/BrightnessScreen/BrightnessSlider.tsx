import React, { useRef, useState, useCallback } from 'react';
import styles from './BrightnessSlider.module.css';

interface BrightnessSliderProps {
  value: number;
  min?: number;
  max?: number;
  step?: number;
  disabled?: boolean;
  onChange: (val: number) => void;
  ariaLabel?: string;
}

export const BrightnessSlider: React.FC<BrightnessSliderProps> = ({
  value,
  min = 0,
  max = 100,
  step = 1,
  disabled = false,
  onChange,
  ariaLabel = 'Brightness',
}) => {
  const trackRef = useRef<HTMLDivElement>(null);
  const [isDragging, setIsDragging] = useState(false);
  const [dragValue, setDragValue] = useState<number | null>(null);

  const displayValue = isDragging && dragValue !== null ? dragValue : value;
  const percentage = Math.min(Math.max(((displayValue - min) / (max - min)) * 100, 0), 100);

  const calculateValueFromPointer = useCallback(
    (clientX: number): number => {
      if (!trackRef.current) return value;
      const rect = trackRef.current.getBoundingClientRect();
      const rawRatio = (clientX - rect.left) / rect.width;
      const clampedRatio = Math.min(Math.max(rawRatio, 0), 1);
      const rawVal = min + clampedRatio * (max - min);
      const steppedVal = Math.round(rawVal / step) * step;
      return Math.min(Math.max(steppedVal, min), max);
    },
    [min, max, step, value]
  );

  const handlePointerDown = (e: React.PointerEvent<HTMLDivElement>) => {
    if (disabled || e.button !== 0) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    setIsDragging(true);
    const newVal = calculateValueFromPointer(e.clientX);
    setDragValue(newVal);
    onChange(newVal);
  };

  const handlePointerMove = (e: React.PointerEvent<HTMLDivElement>) => {
    if (!isDragging || disabled) return;
    const newVal = calculateValueFromPointer(e.clientX);
    if (newVal !== dragValue) {
      setDragValue(newVal);
      onChange(newVal);
    }
  };

  const handlePointerUp = (e: React.PointerEvent<HTMLDivElement>) => {
    if (!isDragging) return;
    try {
      e.currentTarget.releasePointerCapture(e.pointerId);
    } catch {
      // Ignored if already released
    }
    setIsDragging(false);
    setDragValue(null);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (disabled) return;
    let next = value;
    if (e.key === 'ArrowRight' || e.key === 'ArrowUp') {
      next = Math.min(max, value + (e.shiftKey ? step * 5 : step));
    } else if (e.key === 'ArrowLeft' || e.key === 'ArrowDown') {
      next = Math.max(min, value - (e.shiftKey ? step * 5 : step));
    } else if (e.key === 'Home') {
      next = min;
    } else if (e.key === 'End') {
      next = max;
    } else {
      return;
    }
    e.preventDefault();
    onChange(next);
  };

  const handleWheel = useCallback(
    (e: React.WheelEvent) => {
      if (disabled) return;
      e.preventDefault();
      const delta = e.deltaY < 0 ? step : -step;
      const next = Math.min(Math.max(value + delta, min), max);
      if (next !== value) {
        onChange(next);
      }
    },
    [disabled, value, min, max, step, onChange]
  );

  return (
    <div
      ref={trackRef}
      role="slider"
      tabIndex={disabled ? -1 : 0}
      aria-label={ariaLabel}
      aria-valuenow={displayValue}
      aria-valuemin={min}
      aria-valuemax={max}
      className={`${styles.sliderContainer} ${disabled ? styles.disabled : ''} ${isDragging ? styles.dragging : ''}`}
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={handlePointerUp}
      onPointerCancel={handlePointerUp}
      onKeyDown={handleKeyDown}
      onWheel={handleWheel}
    >
      <div className={styles.track}>
        <div
          className={styles.trackFill}
          style={{ width: `${percentage}%` }}
        />
      </div>

      <div
        className={styles.thumbVisual}
        style={{ left: `${percentage}%` }}
      />
    </div>
  );
};
