import type {ReactNode} from 'react';
import styles from './Atom.module.css';

const ORBITS = [0, 60, -60];

export default function Atom({className}: {className?: string}): ReactNode {
  return (
    <svg className={`${styles.atom} ${className ?? ''}`} viewBox="0 0 400 400" role="img" aria-label="Quark atom">
      <defs>
        <linearGradient id="atom-orbit" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#5eead4" />
          <stop offset="0.5" stopColor="#818cf8" />
          <stop offset="1" stopColor="#e879f9" />
        </linearGradient>
        <radialGradient id="atom-core" cx="0.42" cy="0.38" r="0.65">
          <stop offset="0" stopColor="#ffffff" />
          <stop offset="0.22" stopColor="#f0abfc" />
          <stop offset="0.6" stopColor="#8b5cf6" />
          <stop offset="1" stopColor="#3b82f6" />
        </radialGradient>
        <radialGradient id="atom-halo" cx="0.5" cy="0.5" r="0.5">
          <stop offset="0" stopColor="#c084fc" stopOpacity="0.55" />
          <stop offset="1" stopColor="#7c3aed" stopOpacity="0" />
        </radialGradient>
        <radialGradient id="atom-cyan" cx="0.35" cy="0.35" r="0.7">
          <stop offset="0" stopColor="#ecfeff" />
          <stop offset="1" stopColor="#22d3ee" />
        </radialGradient>
        <radialGradient id="atom-pink" cx="0.35" cy="0.35" r="0.7">
          <stop offset="0" stopColor="#fdf4ff" />
          <stop offset="1" stopColor="#d946ef" />
        </radialGradient>
        <filter id="atom-glow" x="-50%" y="-50%" width="200%" height="200%">
          <feGaussianBlur stdDeviation="6" result="blur" />
          <feMerge>
            <feMergeNode in="blur" />
            <feMergeNode in="SourceGraphic" />
          </feMerge>
        </filter>
      </defs>
      <circle cx="200" cy="200" r="150" fill="url(#atom-halo)" className={styles.halo} />
      {ORBITS.map((angle, index) => (
        <g key={angle} transform={`rotate(${angle} 200 200)`}>
          <ellipse
            cx="200"
            cy="200"
            rx="175"
            ry="62"
            fill="none"
            stroke="url(#atom-orbit)"
            strokeWidth="9"
            filter="url(#atom-glow)"
          />
          <g className={styles.electron} style={{animationDelay: `${index * -2.3}s`}}>
            <circle r="16" fill={index % 2 === 0 ? 'url(#atom-pink)' : 'url(#atom-cyan)'} filter="url(#atom-glow)" />
          </g>
        </g>
      ))}
      <circle cx="200" cy="200" r="54" fill="url(#atom-core)" filter="url(#atom-glow)" className={styles.core} />
    </svg>
  );
}
