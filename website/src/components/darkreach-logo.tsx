/**
 * Darkreach logo component. Renders as an inline SVG with an optional
 * purple glow filter for the hero section.
 */

interface DarkReachLogoProps {
  size?: number;
  glow?: boolean;
  className?: string;
}

export function DarkReachLogo({ size = 32, glow = false, className = "" }: DarkReachLogoProps) {
  const id = glow ? "darkreach-glow" : undefined;

  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 100 100"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      className={className}
    >
      {glow && (
        <defs>
          <filter id="glow" x="-50%" y="-50%" width="200%" height="200%">
            <feGaussianBlur stdDeviation="4" result="blur" />
            <feMerge>
              <feMergeNode in="blur" />
              <feMergeNode in="SourceGraphic" />
            </feMerge>
          </filter>
        </defs>
      )}
      <text
        x="50"
        y="78"
        textAnchor="middle"
        fontFamily="Georgia, 'Times New Roman', serif"
        fontSize="90"
        fontWeight="bold"
        fill="#bc8cff"
        filter={glow ? "url(#glow)" : undefined}
      >
        Σ
      </text>
    </svg>
  );
}
