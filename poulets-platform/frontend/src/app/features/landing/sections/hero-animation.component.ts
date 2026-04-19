// SPDX-License-Identifier: AGPL-3.0-or-later
import {
  ChangeDetectionStrategy,
  Component,
  ElementRef,
  HostListener,
  PLATFORM_ID,
  ViewChild,
  inject,
  signal,
} from '@angular/core';
import { CommonModule, isPlatformBrowser } from '@angular/common';
import { TranslateModule } from '@ngx-translate/core';

interface GrowthStage {
  day: string;
  labelKey: string;
  fill: string;
  bodyRx: number;
  bodyRy: number;
  headR: number;
  combColor: string;
  legLen: number;
}

@Component({
  selector: 'app-hero-animation',
  standalone: true,
  imports: [CommonModule, TranslateModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <figure
      class="hero-anim"
      #scene
      role="img"
      [attr.aria-label]="'landing.heroAnim.aria' | translate"
    >
      <svg viewBox="0 0 520 300" xmlns="http://www.w3.org/2000/svg" preserveAspectRatio="xMidYMid meet">
        <defs>
          <linearGradient id="heroAnimSky" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stop-color="#FFE9A8"/>
            <stop offset="100%" stop-color="#FFF7E0"/>
          </linearGradient>
          <linearGradient id="heroAnimGround" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stop-color="#8BC34A"/>
            <stop offset="100%" stop-color="#4A7C2A"/>
          </linearGradient>
          <radialGradient id="heroAnimSun" cx="50%" cy="50%" r="50%">
            <stop offset="0%" stop-color="#FCD116"/>
            <stop offset="100%" stop-color="#FCD116" stop-opacity="0"/>
          </radialGradient>
        </defs>

        <rect x="0" y="0" width="520" height="220" fill="url(#heroAnimSky)" rx="18"/>
        <circle cx="460" cy="60" r="65" fill="url(#heroAnimSun)"/>
        <circle cx="460" cy="60" r="22" fill="#FCD116"/>

        <ellipse cx="260" cy="260" rx="280" ry="60" fill="url(#heroAnimGround)"/>

        <g class="timeline">
          <line x1="60" y1="200" x2="460" y2="200" stroke="#5D4037" stroke-width="2" stroke-dasharray="4 4" opacity="0.35"/>
          @for (stage of stages; track stage.day; let i = $index) {
            <g [attr.transform]="'translate(' + stagePositions[i] + ', 200)'">
              <circle r="4" fill="#EF2B2D"/>
              <text y="22" text-anchor="middle" class="day-label">{{ stage.day }}</text>
              <text y="36" text-anchor="middle" class="day-key">{{ stage.labelKey | translate }}</text>
            </g>
          }
        </g>

        @for (stage of stages; track stage.day; let i = $index) {
          <g
            class="bird"
            [class.bird-walking]="!reducedMotion"
            [attr.transform]="'translate(' + stagePositions[i] + ', ' + birdY(i) + ')'"
            [style.animation-delay]="(i * 0.35) + 's'"
          >
            <ellipse [attr.rx]="stage.bodyRx" [attr.ry]="stage.bodyRy" [attr.fill]="stage.fill"/>
            <circle
              [attr.cx]="-stage.bodyRx * 0.7"
              [attr.cy]="-stage.bodyRy * 0.6"
              [attr.r]="stage.headR"
              [attr.fill]="stage.fill"
            />

            <g class="eye" [attr.transform]="'translate(' + (-stage.bodyRx * 0.75) + ',' + (-stage.bodyRy * 0.7) + ')'">
              <circle r="2.4" fill="#FFFFFF"/>
              <circle
                class="pupil"
                [attr.cx]="pupilOffset().x"
                [attr.cy]="pupilOffset().y"
                r="1.4"
                fill="#1B1B1B"
              />
            </g>

            <polygon
              [attr.points]="beakPoints(stage)"
              fill="#FF8F00"
            />

            @if (i > 0) {
              <polygon
                [attr.points]="combPoints(stage)"
                [attr.fill]="stage.combColor"
              />
            }

            <line
              class="leg leg-l"
              [attr.x1]="-stage.bodyRx * 0.25"
              [attr.y1]="stage.bodyRy * 0.8"
              [attr.x2]="-stage.bodyRx * 0.35"
              [attr.y2]="stage.bodyRy * 0.8 + stage.legLen"
              stroke="#FF8F00"
              stroke-width="2"
              stroke-linecap="round"
            />
            <line
              class="leg leg-r"
              [attr.x1]="stage.bodyRx * 0.25"
              [attr.y1]="stage.bodyRy * 0.8"
              [attr.x2]="stage.bodyRx * 0.35"
              [attr.y2]="stage.bodyRy * 0.8 + stage.legLen"
              stroke="#FF8F00"
              stroke-width="2"
              stroke-linecap="round"
            />
          </g>
        }

        <g class="grain" opacity="0.6">
          @for (g of grainDots; track g.x) {
            <circle [attr.cx]="g.x" [attr.cy]="g.y" r="1.6" fill="#5D4037"/>
          }
        </g>
      </svg>

      <figcaption class="sr-only">{{ 'landing.heroAnim.caption' | translate }}</figcaption>
    </figure>
  `,
  styles: [`
    :host {
      display: block;
      width: 100%;
      max-width: 560px;
      margin-inline: auto;
    }
    .hero-anim {
      position: relative;
      width: 100%;
      margin: 0;
      filter: drop-shadow(0 12px 28px rgba(15, 62, 30, 0.18));
    }
    .hero-anim svg { width: 100%; height: auto; display: block; border-radius: 18px; }

    .day-label {
      font: 700 11px system-ui, -apple-system, Segoe UI, Roboto, sans-serif;
      fill: #2E3A2F;
    }
    .day-key {
      font: 500 9px system-ui, -apple-system, Segoe UI, Roboto, sans-serif;
      fill: #5D4037;
      letter-spacing: 0.04em;
      text-transform: uppercase;
    }

    .bird {
      transform-box: fill-box;
      transform-origin: center;
    }
    .bird-walking {
      animation: bird-bob 1.8s ease-in-out infinite;
    }
    .bird-walking .leg-l {
      transform-origin: top;
      animation: leg-swing-l 0.9s ease-in-out infinite;
    }
    .bird-walking .leg-r {
      transform-origin: top;
      animation: leg-swing-r 0.9s ease-in-out infinite;
    }

    .pupil {
      transition: cx 120ms ease-out, cy 120ms ease-out;
    }

    @keyframes bird-bob {
      0%, 100% { translate: 0 0; }
      50%      { translate: 0 -3px; }
    }
    @keyframes leg-swing-l {
      0%, 100% { transform: rotate(0deg); }
      50%      { transform: rotate(22deg); }
    }
    @keyframes leg-swing-r {
      0%, 100% { transform: rotate(0deg); }
      50%      { transform: rotate(-22deg); }
    }

    .sr-only {
      position: absolute; width: 1px; height: 1px;
      padding: 0; margin: -1px; overflow: hidden;
      clip: rect(0,0,0,0); white-space: nowrap; border: 0;
    }

    @media (prefers-reduced-motion: reduce) {
      .bird-walking, .bird-walking .leg-l, .bird-walking .leg-r {
        animation: none !important;
      }
      .pupil { transition: none; }
    }
  `],
})
export class HeroAnimationComponent {
  private readonly platformId = inject(PLATFORM_ID);
  private readonly host = inject(ElementRef<HTMLElement>);

  @ViewChild('scene', { static: true }) sceneRef!: ElementRef<HTMLElement>;

  readonly reducedMotion = isPlatformBrowser(this.platformId)
    && typeof window !== 'undefined'
    && window.matchMedia?.('(prefers-reduced-motion: reduce)').matches;

  readonly stages: GrowthStage[] = [
    { day: 'J0',  labelKey: 'landing.heroAnim.j0',  fill: '#FDD835', bodyRx: 12, bodyRy: 10, headR: 7,  combColor: '#EF2B2D', legLen: 8  },
    { day: 'J21', labelKey: 'landing.heroAnim.j21', fill: '#F5F1D4', bodyRx: 18, bodyRy: 14, headR: 9,  combColor: '#EF2B2D', legLen: 12 },
    { day: 'J35', labelKey: 'landing.heroAnim.j35', fill: '#E8E0A8', bodyRx: 22, bodyRy: 17, headR: 11, combColor: '#C62828', legLen: 16 },
    { day: 'J45', labelKey: 'landing.heroAnim.j45', fill: '#FFFFFF', bodyRx: 26, bodyRy: 20, headR: 13, combColor: '#B71C1C', legLen: 20 },
  ];

  readonly stagePositions = [100, 220, 340, 440];

  readonly grainDots = [
    { x: 80,  y: 232 }, { x: 130, y: 240 }, { x: 175, y: 228 },
    { x: 240, y: 244 }, { x: 295, y: 234 }, { x: 380, y: 245 },
    { x: 415, y: 232 }, { x: 170, y: 252 }, { x: 300, y: 258 },
  ];

  readonly pupilOffset = signal<{ x: number; y: number }>({ x: 0, y: 0 });

  birdY(i: number): number {
    return 195 - this.stages[i].bodyRy - this.stages[i].legLen;
  }

  beakPoints(s: GrowthStage): string {
    const x = -s.bodyRx * 0.7 - s.headR;
    const y = -s.bodyRy * 0.6;
    return `${x},${y - 1} ${x - 6},${y + 1} ${x},${y + 3}`;
  }

  combPoints(s: GrowthStage): string {
    const x = -s.bodyRx * 0.7;
    const y = -s.bodyRy * 0.6 - s.headR;
    return `${x - 4},${y + 2} ${x - 2},${y - 3} ${x},${y + 1} ${x + 2},${y - 4} ${x + 4},${y}`;
  }

  @HostListener('pointermove', ['$event'])
  onPointerMove(evt: PointerEvent) {
    if (this.reducedMotion) return;
    const el = this.sceneRef?.nativeElement;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    const cx = rect.left + rect.width / 2;
    const cy = rect.top + rect.height / 2;
    const dx = evt.clientX - cx;
    const dy = evt.clientY - cy;
    const mag = Math.hypot(dx, dy) || 1;
    const max = 1.1;
    this.pupilOffset.set({
      x: (dx / mag) * max,
      y: (dy / mag) * max,
    });
  }

  @HostListener('pointerleave')
  onPointerLeave() {
    this.pupilOffset.set({ x: 0, y: 0 });
  }
}
