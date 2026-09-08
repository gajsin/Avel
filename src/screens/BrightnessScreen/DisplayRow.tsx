import React, { useState, useEffect } from 'react';
import { Monitor, Laptop, Minus, Plus } from 'lucide-react';
import { BrightnessDisplay } from '../../types';
import { useApp } from '../../context/AppContext';
import { BrightnessSlider } from './BrightnessSlider';
import styles from './DisplayRow.module.css';

interface DisplayRowProps {
  display: BrightnessDisplay;
  step?: number;
  onBrightnessChange: (displayId: string, percent: number) => void;
}

export const DisplayRow: React.FC<DisplayRowProps> = ({
  display,
  step = 5,
  onBrightnessChange,
}) => {
  const { t } = useApp();
  const currentValue = display.target_percent ?? display.current_percent ?? 50;
  const [inputValue, setInputValue] = useState<string>(currentValue.toString());

  useEffect(() => {
    setInputValue(currentValue.toString());
  }, [currentValue]);

  const isSupported = display.probe_state === 'read_ok';
  const isInternal = display.backend === 'wmi' || display.connection?.toLowerCase().includes('internal') || display.connection?.toLowerCase().includes('edp');

  const handleStepMinus = () => {
    if (!isSupported) return;
    const next = Math.max(0, currentValue - step);
    onBrightnessChange(display.id, next);
  };

  const handleStepPlus = () => {
    if (!isSupported) return;
    const next = Math.min(100, currentValue + step);
    onBrightnessChange(display.id, next);
  };

  const handleInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const raw = e.target.value.replace(/[^0-9]/g, '');
    setInputValue(raw);
  };

  const handleInputBlur = () => {
    const num = parseInt(inputValue, 10);
    if (!isNaN(num)) {
      const clamped = Math.min(Math.max(num, 0), 100);
      setInputValue(clamped.toString());
      if (clamped !== currentValue) {
        onBrightnessChange(display.id, clamped);
      }
    } else {
      setInputValue(currentValue.toString());
    }
  };

  const handleInputKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      handleInputBlur();
    }
  };

  const backendName = display.backend === 'wmi' ? 'WMI' : 'DDC/CI';

  return (
    <div className={styles.displayCard}>
      <div className={styles.header}>
        <div className={styles.titleArea}>
          <div className={styles.iconWrapper}>
            {isInternal ? <Laptop size={18} /> : <Monitor size={18} />}
          </div>
          <div className={styles.titleMeta}>
            <div className={styles.nameRow}>
              <span className={styles.displayName} title={display.name}>
                {display.name}
              </span>
              {display.is_primary && (
                <span className={`${styles.badge} ${styles.badgePrimary}`}>
                  {t.brightness.primaryMonitor}
                </span>
              )}
            </div>
            <div className={styles.badgeList}>
              {display.connection && (
                <span className={styles.badge}>{display.connection}</span>
              )}
              {display.backend && (
                <span className={styles.badge}>{backendName}</span>
              )}
            </div>
          </div>
        </div>
      </div>

      {isSupported ? (
        <div className={styles.sliderRow}>
          <button
            type="button"
            className={styles.stepperBtn}
            onClick={handleStepMinus}
            disabled={!isSupported || currentValue <= 0}
            aria-label="Decrease brightness"
          >
            <Minus size={14} />
          </button>

          <BrightnessSlider
            value={currentValue}
            min={0}
            max={100}
            step={1}
            disabled={!isSupported}
            onChange={(val) => onBrightnessChange(display.id, val)}
            ariaLabel={display.name}
          />

          <button
            type="button"
            className={styles.stepperBtn}
            onClick={handleStepPlus}
            disabled={!isSupported || currentValue >= 100}
            aria-label="Increase brightness"
          >
            <Plus size={14} />
          </button>

          <div className={styles.valueInputWrapper}>
            <input
              type="text"
              className={styles.valueInput}
              value={`${inputValue}%`}
              disabled={!isSupported}
              onChange={handleInputChange}
              onBlur={handleInputBlur}
              onKeyDown={handleInputKeyDown}
              aria-label="Brightness percentage"
            />
          </div>
        </div>
      ) : (
        <p className={styles.unsupportedNote}>
          {t.brightness.noDisplaysDesc}
        </p>
      )}
    </div>
  );
};
