<script lang="ts" module>
  export type AiToolProgressState =
    | "idle"
    | "starting"
    | "thinking"
    | "writing"
    | "working"
    | "waiting"
    | "finished"
    | "stopped"
    | "error";

  export type AiToolProgressVariant = "inline" | "panel" | "pill";
  export type AiToolProgressSize = "sm" | "md" | "lg";
</script>

<script lang="ts">
  import { cn } from "../../utils/cn.js";

  type Props = {
    state?: AiToolProgressState;
    variant?: AiToolProgressVariant;
    size?: AiToolProgressSize;
    label?: string;
    detail?: string;
    toolName?: string;
    showLabel?: boolean;
    class?: string;
  };

  const stateLabel: Record<AiToolProgressState, string> = {
    idle: "Ready",
    starting: "Starting",
    thinking: "Thinking",
    writing: "Writing",
    working: "Working",
    waiting: "Waiting",
    finished: "Finished",
    stopped: "Stopped",
    error: "Needs attention"
  };

  const stateDetail: Record<AiToolProgressState, string> = {
    idle: "Tool is ready",
    starting: "Preparing the next step",
    thinking: "Reading the request",
    writing: "Composing the response",
    working: "Running the selected tool",
    waiting: "Waiting for results",
    finished: "Response complete",
    stopped: "Tool activity has stopped",
    error: "Tool activity paused"
  };

  let {
    state = "idle",
    variant = "inline",
    size = "md",
    label,
    detail,
    toolName,
    showLabel = true,
    class: className
  }: Props = $props();

  const isActive = $derived(
    state === "starting" || state === "thinking" || state === "writing" || state === "working" || state === "waiting"
  );
  const resolvedLabel = $derived(label ?? toolName ?? stateLabel[state]);
  const resolvedDetail = $derived(detail ?? stateDetail[state]);
  const ariaText = $derived(toolName ? `${stateLabel[state]}: ${toolName}. ${resolvedDetail}` : `${stateLabel[state]}. ${resolvedDetail}`);
</script>

<div
  class={cn(
    "ai-tool-progress",
    `ai-tool-progress--${variant}`,
    `ai-tool-progress--${size}`,
    `ai-tool-progress--${state}`,
    className
  )}
  data-state={state}
  role={state === "error" ? "alert" : "status"}
  aria-live={state === "error" ? "assertive" : "polite"}
  aria-label={ariaText}
>
  <span class="ai-tool-progress__mark" aria-hidden="true">
    <span class="ai-tool-progress__halo"></span>
    <svg class="ai-tool-progress__svg" viewBox="0 0 1024 1024" focusable="false">
      <path d="M0 0 C0 4.345 -2.602 6.664 -5.188 9.938 C-21.478 31.128 -34.853 54.553 -47.437 78.083 C-48.577 80.211 -49.727 82.335 -50.882 84.456 C-55.712 93.36 -60.138 102.396 -64.312 111.625 C-64.633 112.331 -64.953 113.037 -65.283 113.764 C-83.285 153.585 -95.748 194.558 -101 238 C-101.105 238.839 -101.209 239.677 -101.317 240.541 C-103.633 260.904 -105.152 288.239 -95 307 C-90.896 311.204 -87.314 312.17 -81.562 312.312 C-16.942 311.668 60.492 264.829 112 229 C112.68 228.53 113.359 228.06 114.06 227.576 C128.291 217.715 141.891 207.309 155 196 C155.74 195.362 156.48 194.724 157.242 194.066 C159.172 192.389 161.088 190.698 163 189 C163.789 188.304 164.578 187.608 165.391 186.891 C171.13 181.785 176.627 176.49 182 171 C182.66 171.33 183.32 171.66 184 172 C183.504 172.485 183.007 172.969 182.496 173.469 C177.153 178.741 171.761 184.081 167.426 190.234 C166.955 190.817 166.485 191.4 166 192 C165.34 192 164.68 192 164 192 C163.796 192.53 163.593 193.06 163.383 193.605 C161.399 197.04 158.699 199.553 155.875 202.312 C155.288 202.894 154.7 203.475 154.095 204.073 C150.472 207.632 146.763 211.042 142.891 214.328 C140.664 216.298 138.588 218.384 136.5 220.5 C132.493 224.548 128.341 228.316 124 232 C123.055 232.811 122.11 233.622 121.137 234.457 C110.092 243.904 98.793 253.027 87.117 261.684 C84.903 263.329 82.701 264.986 80.508 266.66 C69.858 274.767 58.898 282.395 47.885 290 C44.235 292.531 40.605 295.09 36.974 297.65 C29.527 302.897 22.028 308.001 14.332 312.875 C10.813 315.119 7.341 317.425 3.875 319.75 C-9.415 328.642 -22.767 337.424 -36.529 345.574 C-41.894 348.788 -41.894 348.788 -43 351 C-41.766 351.394 -40.533 351.789 -39.262 352.195 C-34.491 353.786 -29.847 355.63 -25.186 357.514 C20.13 375.794 66.906 389.315 115 398 C111.453 400.364 108.104 400.985 104.004 401.812 C102.332 402.163 100.66 402.515 98.988 402.867 C98.547 402.959 98.547 402.959 96.311 403.424 C-71.508 438.578 -71.508 438.578 -107.009 492.473 C-132.156 532.113 -147.002 579.144 -159.5 624.125 C-159.702 624.848 -159.903 625.57 -160.111 626.315 C-162.598 635.274 -164.81 644.25 -166.631 653.371 C-167.026 655.261 -167.505 657.133 -168 659 C-168.66 659.33 -169.32 659.66 -170 660 C-170.14 659.208 -170.28 658.415 -170.424 657.599 C-176.675 622.653 -186.268 588.228 -200.308 555.559 C-200.663 554.731 -201.017 553.904 -201.383 553.051 C-201.54 552.693 -201.54 552.693 -202.333 550.884 C-203 549 -203 549 -203 546 C-210.516 551.768 -217.463 558.012 -224.37 564.479 C-226.918 566.857 -229.49 569.21 -232.062 571.562 C-232.555 572.014 -233.047 572.466 -233.555 572.932 C-236.003 575.178 -238.459 577.414 -240.926 579.641 C-247.316 585.43 -253.462 591.383 -259.457 597.581 C-260.935 599.09 -262.463 600.55 -264 602 C-264.66 602 -265.32 602 -266 602 C-266 602.66 -266 603.32 -266 604 C-266.66 604 -267.32 604 -268 604 C-268 604.66 -268 605.32 -268 606 C-268.66 606 -269.32 606 -270 606 C-270.66 607.32 -271.32 608.64 -272 610 C-272.66 610 -273.32 610 -274 610 C-274 610.66 -274 611.32 -274 612 C-275.619 613.713 -277.295 615.372 -279 617 C-279.817 617.812 -280.634 618.624 -281.476 619.46 C-282.327 620.305 -283.178 621.149 -284.055 622.02 C-285.501 623.458 -286.947 624.898 -288.392 626.337 C-289.376 627.316 -290.361 628.294 -291.347 629.272 C-296.584 634.464 -301.75 639.669 -306.518 645.302 C-308.104 647.12 -309.778 648.811 -311.5 650.5 C-314.514 653.467 -317.299 656.573 -320.059 659.777 C-322.288 662.33 -324.58 664.819 -326.875 667.312 C-330.998 671.828 -334.877 676.488 -338.715 681.246 C-341.084 684.101 -343.549 686.834 -346.062 689.562 C-349.899 693.736 -353.504 698.019 -357.008 702.473 C-359.121 705.153 -361.273 707.798 -363.438 710.438 C-364.19 711.357 -364.943 712.276 -365.719 713.223 C-367.216 715.045 -368.716 716.866 -370.219 718.684 C-381.276 732.097 -391.594 746.08 -402 760 C-402.33 759.34 -402.66 758.68 -403 758 C-401.975 755.359 -401.975 755.359 -400.344 751.965 C-400.051 751.348 -399.757 750.731 -399.455 750.096 C-398.815 748.751 -398.171 747.407 -397.525 746.065 C-395.765 742.412 -394.033 738.746 -392.297 735.082 C-391.937 734.324 -391.576 733.565 -391.205 732.784 C-387.639 725.268 -384.181 717.704 -380.75 710.125 C-380.448 709.458 -380.145 708.79 -379.833 708.103 C-378.238 704.581 -376.644 701.059 -375.051 697.537 C-373.247 693.545 -371.44 689.555 -369.632 685.565 C-364.817 674.937 -360.01 664.304 -355.207 653.671 C-354.008 651.018 -352.809 648.366 -351.609 645.713 C-342.189 624.885 -332.835 604.035 -323.859 583.01 C-322.948 580.879 -322.034 578.75 -321.119 576.622 C-313.532 558.959 -306.178 541.231 -299.375 523.25 C-299.093 522.507 -298.811 521.763 -298.52 520.997 C-287.358 491.487 -287.358 491.487 -284.625 478.25 C-284.461 477.523 -284.298 476.796 -284.129 476.047 C-281.929 465.842 -281.929 465.842 -284.508 461.406 C-287.681 458.342 -290.961 456.488 -294.875 454.5 C-295.695 454.078 -296.515 453.655 -297.361 453.22 C-350.476 426.428 -408.446 409.618 -467 400 C-467 399.34 -467 398.68 -467 398 C-466.062 397.835 -465.125 397.67 -464.159 397.5 C-406.676 387.315 -351.623 370.46 -299 345 C-298.51 344.764 -298.51 344.764 -296.028 343.57 C-252.575 322.576 -235.116 299.108 -214.078 256.019 C-206.369 240.262 -198.035 225.036 -189 210 C-188.483 209.137 -187.967 208.274 -187.435 207.385 C-181.107 196.816 -174.469 186.492 -167.555 176.297 C-165.975 173.963 -164.409 171.621 -162.844 169.277 C-155.854 158.869 -148.445 148.89 -140.646 139.075 C-139.084 137.106 -137.534 135.13 -135.988 133.148 C-128.142 123.094 -120.215 113.175 -111.555 103.801 C-109.538 101.59 -107.559 99.358 -105.594 97.102 C-95.556 85.6 -84.841 74.74 -74.062 63.938 C-73.794 63.667 -73.794 63.667 -72.433 62.3 C-67.017 56.864 -61.587 51.495 -55.75 46.505 C-52.986 44.128 -50.346 41.619 -47.688 39.125 C-42.868 34.641 -37.925 30.384 -32.789 26.262 C-30.312 24.253 -27.883 22.197 -25.466 20.117 C-17.248 13.056 -8.656 6.512 0 0 Z" transform="translate(689 131)" />
    </svg>
  </span>

  {#if showLabel}
    <span class="ai-tool-progress__copy">
      <span class="ai-tool-progress__label">{resolvedLabel}</span>
      {#if variant === "panel"}
        <span class="ai-tool-progress__detail">{resolvedDetail}</span>
      {/if}
    </span>
  {/if}

  {#if isActive}
    <span class="ai-tool-progress__beats" aria-hidden="true">
      <span></span>
      <span></span>
      <span></span>
    </span>
  {/if}
</div>

<style>
  .ai-tool-progress {
    --mark-size: 1.25rem;
    --mark-color-rest: var(--epifly-logo-black);
    --mark-color-active: var(--epifly-logo-orange);
    --mark-glow: color-mix(in oklch, var(--epifly-logo-orange) 36%, transparent);

    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    min-width: 0;
    color: var(--mark-color-rest);
    line-height: 1;
  }

  :global(.dark) .ai-tool-progress {
    --mark-color-rest: color-mix(in oklch, var(--foreground) 86%, var(--epifly-logo-black));
  }

  .ai-tool-progress--sm {
    --mark-size: 0.875rem;
    gap: 0.375rem;
  }

  .ai-tool-progress--lg {
    --mark-size: 1.75rem;
    gap: 0.625rem;
  }

  .ai-tool-progress--panel {
    width: fit-content;
    max-width: 100%;
    border: 1px solid var(--epifly-tool-border);
    border-radius: 1rem;
    background: var(--epifly-tool-surface);
    box-shadow: var(--epifly-tool-shadow);
    padding: 0.625rem 0.75rem;
    backdrop-filter: blur(14px);
  }

  .ai-tool-progress--pill {
    --mark-size: 0.72rem;

    width: fit-content;
    max-width: min(100%, 14rem);
    gap: 0.38rem;
    border: 1px solid color-mix(in oklch, var(--border) 76%, transparent);
    border-radius: 999px;
    background: color-mix(in oklch, var(--background) 92%, white);
    box-shadow:
      0 0.0625rem 0.125rem color-mix(in oklch, var(--foreground) 6%, transparent),
      0 0.75rem 1.75rem color-mix(in oklch, var(--foreground) 7%, transparent);
    padding: 0.42rem 0.58rem;
    backdrop-filter: blur(18px) saturate(1.08);
  }

  :global(.dark) .ai-tool-progress--pill {
    border-color: color-mix(in oklch, var(--border) 70%, transparent);
    background: color-mix(in oklch, var(--background) 86%, var(--foreground) 5%);
    box-shadow:
      0 0.0625rem 0 color-mix(in oklch, var(--foreground) 10%, transparent),
      0 0.75rem 1.75rem color-mix(in oklch, black 24%, transparent);
  }

  .ai-tool-progress__mark {
    position: relative;
    display: inline-flex;
    width: var(--mark-size);
    height: var(--mark-size);
    flex: 0 0 auto;
    align-items: center;
    justify-content: center;
  }

  .ai-tool-progress__halo {
    position: absolute;
    inset: -0.3125rem;
    border-radius: 999px;
    background:
      radial-gradient(circle, var(--mark-glow), transparent 64%),
      radial-gradient(circle at 70% 28%, color-mix(in oklch, var(--epifly-tool-cyan) 26%, transparent), transparent 58%);
    opacity: 0;
    transform: scale(0.72);
  }

  .ai-tool-progress--pill .ai-tool-progress__halo {
    inset: -0.25rem;
    opacity: 0;
  }

  .ai-tool-progress__svg {
    position: relative;
    width: 100%;
    height: 100%;
    overflow: visible;
    color: currentColor;
    fill: currentColor;
    transform-origin: 50% 50%;
    transition:
      color var(--motion-base) var(--ease-standard),
      opacity var(--motion-base) var(--ease-standard),
      transform var(--motion-base) var(--ease-standard),
      filter var(--motion-base) var(--ease-standard);
  }

  .ai-tool-progress__svg path {
    fill: currentColor;
  }

  .ai-tool-progress__copy {
    display: inline-flex;
    min-width: 0;
    flex-direction: column;
    gap: 0.0625rem;
  }

  .ai-tool-progress__label {
    overflow: hidden;
    max-width: 16rem;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 0.75rem;
    font-weight: 600;
    line-height: 1.25;
    color: var(--foreground);
  }

  .ai-tool-progress--pill .ai-tool-progress__label {
    max-width: 8.5rem;
    font-size: 0.72rem;
    font-weight: 650;
    line-height: 1;
  }

  .ai-tool-progress--sm .ai-tool-progress__label {
    max-width: 11rem;
    font-size: 0.6875rem;
  }

  .ai-tool-progress__detail {
    overflow: hidden;
    max-width: 18rem;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 0.6875rem;
    line-height: 1.25;
    color: var(--muted-foreground);
  }

  .ai-tool-progress__beats {
    display: inline-flex;
    align-items: center;
    gap: 0.1875rem;
  }

  .ai-tool-progress--pill .ai-tool-progress__beats {
    gap: 0.14rem;
    margin-left: 0.02rem;
  }

  .ai-tool-progress__beats span {
    width: 0.1875rem;
    height: 0.1875rem;
    border-radius: 999px;
    background: currentColor;
    opacity: 0.42;
    animation: ai-tool-beat 1.18s var(--ease-standard) infinite;
  }

  .ai-tool-progress--pill .ai-tool-progress__beats span {
    width: 0.14rem;
    height: 0.14rem;
    opacity: 0.34;
  }

  .ai-tool-progress__beats span:nth-child(2) {
    animation-delay: 140ms;
  }

  .ai-tool-progress__beats span:nth-child(3) {
    animation-delay: 280ms;
  }

  .ai-tool-progress--starting,
  .ai-tool-progress--thinking,
  .ai-tool-progress--writing,
  .ai-tool-progress--working,
  .ai-tool-progress--waiting {
    color: var(--mark-color-active);
  }

  .ai-tool-progress--starting .ai-tool-progress__svg {
    animation: ai-tool-start 680ms var(--ease-emphasized) both;
  }

  .ai-tool-progress--thinking .ai-tool-progress__svg {
    animation: ai-tool-think 2.4s var(--ease-standard) infinite;
  }

  .ai-tool-progress--writing .ai-tool-progress__svg {
    animation: ai-tool-write 1.12s var(--ease-standard) infinite;
  }

  .ai-tool-progress--working .ai-tool-progress__svg {
    animation: ai-tool-work 1.26s var(--ease-standard) infinite;
  }

  .ai-tool-progress--waiting .ai-tool-progress__svg {
    animation: ai-tool-wait 1.9s var(--ease-standard) infinite;
  }

  .ai-tool-progress--starting .ai-tool-progress__halo,
  .ai-tool-progress--thinking .ai-tool-progress__halo,
  .ai-tool-progress--writing .ai-tool-progress__halo,
  .ai-tool-progress--working .ai-tool-progress__halo,
  .ai-tool-progress--waiting .ai-tool-progress__halo {
    animation: ai-tool-halo 1.8s var(--ease-standard) infinite;
  }

  .ai-tool-progress--finished {
    color: color-mix(in oklch, var(--mark-color-active) 72%, var(--foreground));
  }

  .ai-tool-progress--stopped {
    color: color-mix(in oklch, var(--muted-foreground) 78%, var(--mark-color-rest));
  }

  .ai-tool-progress--error {
    color: var(--destructive);
  }

  .ai-tool-progress--error .ai-tool-progress__label {
    color: var(--destructive);
  }

  @keyframes ai-tool-start {
    0% {
      color: var(--mark-color-active);
      filter: none;
      opacity: 0.7;
      transform: scale(0.88) rotate(-3deg);
    }

    48% {
      color: var(--epifly-logo-orange-hot);
      filter: drop-shadow(0 0 0.625rem var(--mark-glow));
      opacity: 1;
      transform: scale(1.05) rotate(2deg);
    }

    100% {
      color: var(--mark-color-active);
      filter: drop-shadow(0 0 0.25rem color-mix(in oklch, var(--epifly-logo-orange) 20%, transparent));
      opacity: 1;
      transform: scale(1) rotate(0deg);
    }
  }

  @keyframes ai-tool-think {
    0%,
    100% {
      color: color-mix(in oklch, var(--epifly-logo-orange) 82%, var(--epifly-logo-orange-hot));
      filter: drop-shadow(0 0 0 color-mix(in oklch, var(--epifly-logo-orange) 0%, transparent));
      transform: translateY(0) scale(0.98) rotate(-1deg);
    }

    50% {
      color: var(--epifly-logo-orange-hot);
      filter: drop-shadow(0 0 0.5rem var(--mark-glow));
      transform: translateY(-0.125rem) scale(1.03) rotate(1deg);
    }
  }

  @keyframes ai-tool-work {
    0%,
    100% {
      color: var(--mark-color-active);
      filter: none;
      transform: scale(0.98);
    }

    42%,
    68% {
      color: var(--epifly-logo-orange-hot);
      filter: drop-shadow(0 0 0.55rem var(--mark-glow));
      transform: scale(1.04);
    }
  }

  @keyframes ai-tool-write {
    0%,
    100% {
      color: var(--mark-color-active);
      filter: drop-shadow(0 0 0.125rem color-mix(in oklch, var(--epifly-logo-orange) 14%, transparent));
      transform: translateX(0) scale(0.99);
    }

    45% {
      color: var(--epifly-logo-orange-hot);
      filter: drop-shadow(0 0 0.45rem var(--mark-glow));
      transform: translateX(0.0625rem) scale(1.03);
    }
  }


  @keyframes ai-tool-wait {
    0%,
    100% {
      color: color-mix(in oklch, var(--mark-color-active) 46%, var(--muted-foreground));
      opacity: 0.64;
      transform: scale(0.98);
    }

    50% {
      color: var(--mark-color-active);
      opacity: 1;
      transform: scale(1.01);
    }
  }

  @keyframes ai-tool-halo {
    0%,
    100% {
      opacity: 0;
      transform: scale(0.7);
    }

    45% {
      opacity: 0.72;
      transform: scale(1);
    }
  }

  @keyframes ai-tool-beat {
    0%,
    70%,
    100% {
      opacity: 0.28;
      transform: translateY(0);
    }

    35% {
      opacity: 0.95;
      transform: translateY(-0.125rem);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .ai-tool-progress__svg,
    .ai-tool-progress__halo,
    .ai-tool-progress__beats span {
      animation: none;
    }

    .ai-tool-progress--starting,
    .ai-tool-progress--thinking,
    .ai-tool-progress--writing,
    .ai-tool-progress--working,
    .ai-tool-progress--waiting {
      color: var(--mark-color-active);
    }

    .ai-tool-progress--starting .ai-tool-progress__halo,
    .ai-tool-progress--thinking .ai-tool-progress__halo,
    .ai-tool-progress--writing .ai-tool-progress__halo,
    .ai-tool-progress--working .ai-tool-progress__halo,
    .ai-tool-progress--waiting .ai-tool-progress__halo {
      opacity: 0.28;
      transform: scale(1);
    }
  }
</style>
