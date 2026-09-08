import React from 'react';
import logoLight from '../../assets/logo-light.svg';
import logoDark from '../../assets/logo-dark.svg';
import markLight from '../../assets/mark-light.svg';
import markDark from '../../assets/mark-dark.svg';
import styles from './AvelLogo.module.css';

interface AvelLogoProps {
  mode?: 'full' | 'mark';
  theme?: 'light' | 'dark';
  className?: string;
  size?: number;
}

export const AvelLogo: React.FC<AvelLogoProps> = ({
  mode = 'full',
  theme = 'light',
  className = '',
  size,
}) => {
  const isDark = theme === 'dark';
  const logoSrc = isDark ? logoDark : logoLight;
  const markSrc = isDark ? markDark : markLight;
  const src = mode === 'mark' ? markSrc : logoSrc;

  return (
    <img
      src={src}
      alt="Avel"
      className={`${styles.logoImg} ${className}`}
      style={size ? { width: size, height: 'auto' } : undefined}
      draggable={false}
    />
  );
};
