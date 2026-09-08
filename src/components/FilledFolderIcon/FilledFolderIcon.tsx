import React from 'react';

export const FilledFolderIcon: React.FC<{ size?: number }> = ({ size = 64 }) => (
  <svg
    width={size}
    height={size}
    viewBox="0 0 64 64"
    fill="none"
    xmlns="http://www.w3.org/2000/svg"
    style={{ filter: 'drop-shadow(0 2px 5px rgba(0,0,0,0.12))' }}
  >
    <defs>
      <linearGradient id="folder_back_grad" x1="8" y1="12" x2="56" y2="48" gradientUnits="userSpaceOnUse">
        <stop offset="0%" stopColor="#F59E0B" />
        <stop offset="100%" stopColor="#D97706" />
      </linearGradient>
      <linearGradient id="folder_front_grad" x1="6" y1="22" x2="58" y2="52" gradientUnits="userSpaceOnUse">
        <stop offset="0%" stopColor="#FCD34D" />
        <stop offset="40%" stopColor="#FBBF24" />
        <stop offset="100%" stopColor="#F59E0B" />
      </linearGradient>
    </defs>
    {/* Back tab & body */}
    <path
      d="M8 16C8 13.7909 9.79086 12 12 12H24.5858C25.6466 12 26.664 12.4214 27.4142 13.1716L31.8284 17.5858C32.5786 18.336 33.596 18.7574 34.6569 18.7574H52C54.2091 18.7574 56 20.5482 56 22.7574V44C56 46.2091 54.2091 48 52 48H12C9.79086 48 8 46.2091 8 44V16Z"
      fill="url(#folder_back_grad)"
    />
    {/* Sheet inside */}
    <rect x="14" y="18" width="36" height="18" rx="2" fill="#FEF3C7" opacity="0.8" />
    {/* Front folder flap */}
    <path
      d="M6 26C6 23.7909 7.79086 22 10 22H54C56.2091 22 58 23.7909 58 26V46C58 48.2091 56.2091 50 54 50H10C7.79086 50 6 48.2091 6 46V26Z"
      fill="url(#folder_front_grad)"
    />
    {/* Top subtle border highlight */}
    <path
      d="M10 23H54C55.6569 23 57 24.3431 57 26V27C57 25.3431 55.6569 24 54 24H10C8.34315 24 7 25.3431 7 27V26C7 24.3431 8.34315 23 10 23Z"
      fill="#FFFFFF"
      opacity="0.4"
    />
  </svg>
);
