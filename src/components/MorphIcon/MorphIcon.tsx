import React from 'react';
import { MorphIcon as RawMorphIcon, type MorphIconProps } from 'morphicons/react';

export interface AppMorphIconProps extends MorphIconProps {
  size?: number | string;
  className?: string;
}

export const MorphIcon = React.forwardRef<any, AppMorphIconProps>(
  (
    {
      size = 16,
      strokeWidth = 2,
      spring = 'smooth',
      reducedMotion = 'never',
      style,
      ...rest
    },
    ref
  ) => {
    return (
      <RawMorphIcon
        ref={ref}
        size={size}
        strokeWidth={strokeWidth}
        spring={spring}
        reducedMotion={reducedMotion}
        style={{
          display: 'inline-block',
          verticalAlign: 'middle',
          flexShrink: 0,
          ...style,
        }}
        {...rest}
      />
    );
  }
);

MorphIcon.displayName = 'MorphIcon';
