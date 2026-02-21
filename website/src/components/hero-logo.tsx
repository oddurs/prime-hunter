"use client";

import { useEffect, useRef, useState } from "react";

/**
 * Animated infinity-mark logo for the hero section.
 * 1. Stroke draws on over 1.8s
 * 2. Fill fades in over 0.8s
 * 3. Continuous soft glow breathing
 */
export function HeroLogo({ size = 100 }: { size?: number }) {
  const pathRef = useRef<SVGPathElement>(null);
  const [drawn, setDrawn] = useState(false);
  const [filled, setFilled] = useState(false);

  const PATH =
    "m38.25 35.93c1.0547 0.69922 1.3438 2.1172 0.64453 3.1758-0.69922 1.0547-2.1172 1.3438-3.1758 0.64453-9.5586-6.3203-18.051-4.9102-18.051 10.25 0 26.418 22.184 7.3438 30.672-1.5781 8.6172-9.0586 17.23-18.117 33.125-20.562 3.7773-0.58203 7.457 0.35938 10.402 2.3867 2.9492 2.0234 5.1523 5.1289 5.9609 8.8711 0.74219 3.4062 1.1133 7.082 1.1211 10.777 0.007812 3.6836-0.35547 7.3633-1.0781 10.789-0.8125 3.832-3.082 6.9922-6.1094 9.0234-3.0234 2.0273-6.8086 2.9219-10.66 2.2227-6.9648-1.2695-13.457-3.957-19.355-7.8594-1.0547-0.69922-1.3438-2.1172-0.64453-3.1758 0.69922-1.0547 2.1211-1.3438 3.1758-0.64453 9.6133 6.3555 18.055 5.2188 18.055-10.25 0-26.418-22.184-7.3438-30.672 1.5781-8.6172 9.0586-17.23 18.117-33.125 20.562-3.7734 0.58203-7.4531-0.35938-10.402-2.3867-2.9492-2.0234-5.1484-5.1289-5.9609-8.8711-0.74219-3.4062-1.1172-7.082-1.1211-10.777-0.007812-3.6836 0.35156-7.3633 1.0781-10.789 0.8125-3.832 3.082-6.9922 6.1094-9.0234 3.0234-2.0273 6.8086-2.9219 10.66-2.2227 3.9102 0.71094 7.4805 1.8398 10.707 3.207 3.2422 1.375 6.125 2.9805 8.6484 4.6484z";

  useEffect(() => {
    const path = pathRef.current;
    if (!path) return;

    // Measure path length and set up stroke-dasharray
    const length = path.getTotalLength();
    path.style.strokeDasharray = `${length}`;
    path.style.strokeDashoffset = `${length}`;

    // Trigger draw animation on next frame
    requestAnimationFrame(() => {
      path.style.transition = "stroke-dashoffset 1.8s cubic-bezier(0.4, 0, 0.2, 1)";
      path.style.strokeDashoffset = "0";
    });

    const drawTimer = setTimeout(() => setDrawn(true), 1800);
    const fillTimer = setTimeout(() => setFilled(true), 2200);

    return () => {
      clearTimeout(drawTimer);
      clearTimeout(fillTimer);
    };
  }, []);

  const height = size * 0.72;

  return (
    <div className="relative" style={{ width: size, height }}>
      {/* Breathing glow behind the logo */}
      <div
        className="absolute inset-0 rounded-full hero-logo-glow"
        style={{
          background:
            "radial-gradient(circle, rgba(99,102,241,0.3) 0%, rgba(99,102,241,0.08) 40%, transparent 70%)",
          transform: "scale(2.5)",
        }}
      />

      <svg
        width={size}
        height={height}
        viewBox="-2 14 104 72"
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
        className="relative"
        aria-label="darkreach logo"
      >
        <defs>
          <linearGradient id="hero-logo-grad" x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" stopColor="#818cf8" />
            <stop offset="50%" stopColor="#6366f1" />
            <stop offset="100%" stopColor="#a78bfa" />
          </linearGradient>
          <filter id="hero-logo-glow" x="-50%" y="-50%" width="200%" height="200%">
            <feGaussianBlur stdDeviation="4" result="blur" />
            <feFlood floodColor="#6366f1" floodOpacity="0.6" result="color" />
            <feComposite in="color" in2="blur" operator="in" result="glow" />
            <feMerge>
              <feMergeNode in="glow" />
              <feMergeNode in="SourceGraphic" />
            </feMerge>
          </filter>
        </defs>

        {/* Stroke draw-on path */}
        <path
          ref={pathRef}
          d={PATH}
          fill="none"
          stroke="url(#hero-logo-grad)"
          strokeWidth={2.5}
          strokeLinecap="round"
          strokeLinejoin="round"
          style={{ opacity: drawn && filled ? 0 : 1, transition: "opacity 0.6s ease" }}
        />

        {/* Filled path fades in after stroke completes */}
        <path
          d={PATH}
          fill="url(#hero-logo-grad)"
          filter="url(#hero-logo-glow)"
          style={{
            opacity: filled ? 1 : 0,
            transition: "opacity 0.8s ease",
          }}
        />
      </svg>
    </div>
  );
}
