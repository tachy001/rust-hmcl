<template>
  <div class="world-scene" aria-hidden="true">
    <svg viewBox="0 0 560 360" fill="none">
      <defs>
        <linearGradient
          id="island-top"
          x1="160"
          y1="130"
          x2="360"
          y2="260"
          gradientUnits="userSpaceOnUse"
        >
          <stop stop-color="#72d6ba" />
          <stop offset="1" stop-color="#276e68" />
        </linearGradient>
        <linearGradient
          id="island-side"
          x1="280"
          y1="210"
          x2="280"
          y2="320"
          gradientUnits="userSpaceOnUse"
        >
          <stop stop-color="#204d50" />
          <stop offset="1" stop-color="#122631" />
        </linearGradient>
        <radialGradient id="scene-glow">
          <stop stop-color="#5ce1c5" stop-opacity=".18" />
          <stop offset="1" stop-color="#5ce1c5" stop-opacity="0" />
        </radialGradient>
        <linearGradient id="portal-glow">
          <stop stop-color="#e0fda4" />
          <stop offset="1" stop-color="#64dcc4" />
        </linearGradient>
      </defs>
      <ellipse cx="300" cy="219" rx="240" ry="137" fill="url(#scene-glow)" />
      <g class="scene-orbit" stroke="#77bdb5" stroke-opacity=".22">
        <ellipse cx="305" cy="217" rx="201" ry="68" transform="rotate(-18 305 217)" />
        <ellipse
          cx="305"
          cy="217"
          rx="230"
          ry="92"
          transform="rotate(-18 305 217)"
          stroke-dasharray="2 8"
        />
      </g>
      <g class="scene-route" stroke="#a5e8d2" stroke-width="1.5" stroke-linecap="round">
        <path d="M110 246C165 315 422 271 496 166" pathLength="100" stroke-dasharray="5 95" />
        <path d="M150 156C215 88 430 109 474 230" pathLength="100" stroke-dasharray="3 97" />
      </g>
      <circle cx="425" cy="75" r="34" fill="#d1e9b3" opacity=".8" />
      <circle cx="437" cy="65" r="30" fill="#102a34" />
      <g class="scene-stars" fill="#b0e2d8">
        <path d="m149 71 2 7 7 2-7 2-2 7-2-7-7-2 7-2Z" />
        <path d="m460 181 2 5 5 2-5 2-2 5-2-5-5-2 5-2Z" />
        <circle cx="338" cy="54" r="2" />
        <circle cx="197" cy="116" r="1.5" />
        <circle cx="495" cy="112" r="1.5" />
        <circle cx="102" cy="198" r="2" />
        <circle cx="380" cy="319" r="1.5" />
      </g>
      <g class="floating-island">
        <path
          d="m153 210 76-47 129 5 86 54-53 50-64 33-47 27-48-35-38-9-31-45Z"
          fill="url(#island-side)"
        />
        <path d="m153 210 76-47 129 5 86 54-66 42-106 12-76-29Z" fill="url(#island-top)" />
        <path
          d="m153 210 43 37 76 29 106-12 66-42"
          stroke="#9ce7c7"
          stroke-opacity=".5"
          stroke-width="2"
        />
        <path
          d="m196 247 2 30 34 20 40-21v37m106-49-7 28-44 13m-98-142 10 54 34 13 85-62"
          stroke="#6caaa0"
          stroke-opacity=".18"
          stroke-width="2"
        />
        <path d="m261 224 15-5 60 23-18 10Z" fill="#d8e6ae" opacity=".7" />
        <path d="m276 219-9-18 8-22 20-13 17 4-21 16-8 17 10 23" fill="#b8d6aa" opacity=".55" />
        <g class="scene-portal">
          <path class="portal-light" d="m334 135 12-7 14 8v62l-12 7-15-11Z" fill="#d6ffc3" />
          <path d="m320 202 1-75 24-14 27 15v75l-11 6v-75l-16-9-14 8v75Z" fill="#234b49" />
          <path d="m333 194 1-59 12-7 14 8v62l-12 7Z" fill="url(#portal-glow)" opacity=".8" />
          <path d="m321 127 24-14 27 15-12 8-14-8-12 7Z" fill="#5a8e78" />
        </g>
        <g>
          <path d="M225 186v35l7 4v-36Z" fill="#364a3d" />
          <path d="m230 112-27 48h12l-23 33 38 16 36-21-22-27h12Z" fill="#24655c" />
          <path d="m230 112 1 95 35-19-22-27h12Z" fill="#368975" />
          <path d="m230 137-18 31 18 8 19-10Z" fill="#5aa68c" opacity=".4" />
        </g>
        <g>
          <path d="M186 197v22l5 3v-22Z" fill="#364a3d" />
          <path d="m190 147-18 29h9l-16 24 25 10 25-13-14-21h7Z" fill="#387c6a" />
          <path d="m190 147 1 62 24-12-14-21h7Z" fill="#50a085" />
        </g>
        <path d="m380 213 14-8 13 9-14 8Z" fill="#a6d9b0" />
        <path d="m393 222 14-8v13l-14 7Z" fill="#345c54" />
        <path d="m380 213 13 9v12l-13-9Z" fill="#527d62" />
        <path d="m315 292 12 13-47 27-17-14 17-15" fill="#123339" />
      </g>
      <g class="scene-satellite">
        <path d="m426 284 22-13 23 13-22 14Z" fill="#5c9e8a" />
        <path d="m426 284 23 14 22-14-22 37Z" fill="#234b4d" />
      </g>
    </svg>
  </div>
</template>
