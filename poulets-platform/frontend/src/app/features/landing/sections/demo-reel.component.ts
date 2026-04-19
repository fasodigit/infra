// SPDX-License-Identifier: AGPL-3.0-or-later
import {
  ChangeDetectionStrategy,
  Component,
  DestroyRef,
  NgZone,
  OnInit,
  PLATFORM_ID,
  computed,
  inject,
  signal,
} from '@angular/core';
import { CommonModule, isPlatformBrowser } from '@angular/common';
import { TranslateModule } from '@ngx-translate/core';

interface Scene {
  id: string;
  iconPath: string;
  titleKey: string;
  captionKey: string;
  accent: string;
}

@Component({
  selector: 'app-demo-reel',
  standalone: true,
  imports: [CommonModule, TranslateModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="demo-reel" id="demo-reel" aria-labelledby="demo-reel-title">
      <div class="demo-inner">
        <header class="demo-head">
          <span class="eyebrow">{{ 'landing.demoReel.eyebrow' | translate }}</span>
          <h2 id="demo-reel-title">{{ 'landing.demoReel.title' | translate }}</h2>
          <p class="lead">{{ 'landing.demoReel.lead' | translate }}</p>
        </header>

        <div
          class="browser"
          (pointerenter)="pause()"
          (pointerleave)="resume()"
          (focusin)="pause()"
          (focusout)="resume()"
        >
          <div class="browser-chrome" aria-hidden="true">
            <div class="dots">
              <span class="dot d-r"></span>
              <span class="dot d-y"></span>
              <span class="dot d-g"></span>
            </div>
            <div class="url">
              <span class="lock">🔒</span>
              <span class="url-text">poulets.faso.bf{{ currentScene().urlPath }}</span>
            </div>
            <div class="spacer"></div>
          </div>

          <div class="stage" [attr.aria-live]="'polite'">
            @for (scene of scenes; track scene.id; let i = $index) {
              <article
                class="scene"
                [class.active]="i === activeIndex()"
                [attr.aria-hidden]="i !== activeIndex()"
              >
                <ng-container [ngSwitch]="scene.id">
                  <ng-container *ngSwitchCase="'search'">
                    <ng-container *ngTemplateOutlet="searchTpl; context: { scene }"/>
                  </ng-container>
                  <ng-container *ngSwitchCase="'listing'">
                    <ng-container *ngTemplateOutlet="listingTpl; context: { scene }"/>
                  </ng-container>
                  <ng-container *ngSwitchCase="'payment'">
                    <ng-container *ngTemplateOutlet="paymentTpl; context: { scene }"/>
                  </ng-container>
                  <ng-container *ngSwitchCase="'delivery'">
                    <ng-container *ngTemplateOutlet="deliveryTpl; context: { scene }"/>
                  </ng-container>
                  <ng-container *ngSwitchCase="'dashboard'">
                    <ng-container *ngTemplateOutlet="dashboardTpl; context: { scene }"/>
                  </ng-container>
                </ng-container>
              </article>
            }
          </div>

          <div class="progress" role="tablist">
            @for (scene of scenes; track scene.id; let i = $index) {
              <button
                role="tab"
                class="prog-dot"
                [class.active]="i === activeIndex()"
                [attr.aria-selected]="i === activeIndex()"
                [attr.aria-label]="scene.titleKey | translate"
                (click)="jumpTo(i)"
              >
                <span class="fill" [style.animation-play-state]="isPaused() || i !== activeIndex() ? 'paused' : 'running'"></span>
              </button>
            }
          </div>
        </div>

        <p class="caption">
          <strong>{{ currentScene().titleKey | translate }}</strong>
          <span class="sep">—</span>
          {{ currentScene().captionKey | translate }}
        </p>
      </div>
    </section>

    <!-- ======== SCENE TEMPLATES ======== -->

    <ng-template #searchTpl let-scene="scene">
      <div class="s-search">
        <div class="searchbar">
          <span class="icon">🔍</span>
          <span class="input-ghost">{{ 'landing.demoReel.search.query' | translate }}</span>
          <span class="chip chip-accent">{{ 'landing.demoReel.search.chipRace' | translate }}</span>
          <span class="chip">{{ 'landing.demoReel.search.chipRegion' | translate }}</span>
        </div>
        <div class="grid-cards">
          @for (c of searchCards; track c.id) {
            <div class="mini-card">
              <div class="mini-thumb" [style.background]="c.bg"></div>
              <div class="mini-body">
                <div class="mini-title">{{ c.titleKey | translate }}</div>
                <div class="mini-meta">
                  <span class="mini-price">{{ c.price }}</span>
                  <span class="mini-badge">★ {{ c.rating }}</span>
                </div>
              </div>
            </div>
          }
        </div>
      </div>
    </ng-template>

    <ng-template #listingTpl let-scene="scene">
      <div class="s-listing">
        <div class="listing-hero" [style.background]="'linear-gradient(135deg, #FFE9A8 0%, #FCD116 100%)'">
          <div class="hero-badges">
            <span class="pill pill-halal">✓ {{ 'landing.demoReel.listing.halal' | translate }}</span>
            <span class="pill pill-vet">🩺 {{ 'landing.demoReel.listing.vet' | translate }}</span>
            <span class="pill pill-local">🇧🇫 {{ 'landing.demoReel.listing.local' | translate }}</span>
          </div>
          <div class="hero-chick">🐔</div>
        </div>
        <div class="listing-body">
          <div class="listing-title">{{ 'landing.demoReel.listing.title' | translate }}</div>
          <div class="listing-row">
            <span class="kv"><b>{{ 'landing.demoReel.listing.weight' | translate }}</b> 2.3 kg</span>
            <span class="kv"><b>{{ 'landing.demoReel.listing.age' | translate }}</b> 45 j</span>
            <span class="kv"><b>{{ 'landing.demoReel.listing.race' | translate }}</b> Sasso</span>
          </div>
          <div class="listing-cta">
            <span class="price-big">4 500 <small>FCFA</small></span>
            <span class="cta-btn">{{ 'landing.demoReel.listing.cta' | translate }}</span>
          </div>
        </div>
      </div>
    </ng-template>

    <ng-template #paymentTpl let-scene="scene">
      <div class="s-payment">
        <div class="pay-col">
          <div class="pay-title">{{ 'landing.demoReel.payment.title' | translate }}</div>
          <div class="pay-method active">
            <span class="pm-logo pm-om">OM</span>
            <span>Orange Money</span>
            <span class="check">✓</span>
          </div>
          <div class="pay-method">
            <span class="pm-logo pm-mm">MM</span>
            <span>Moov Money</span>
          </div>
          <div class="pay-method">
            <span class="pm-logo pm-card">💳</span>
            <span>{{ 'landing.demoReel.payment.card' | translate }}</span>
          </div>
        </div>
        <div class="pay-col receipt">
          <div class="rcp-row"><span>{{ 'landing.demoReel.payment.product' | translate }}</span><b>4 500</b></div>
          <div class="rcp-row"><span>{{ 'landing.demoReel.payment.delivery' | translate }}</span><b>500</b></div>
          <div class="rcp-row total"><span>{{ 'landing.demoReel.payment.total' | translate }}</span><b>5 000 FCFA</b></div>
          <div class="pay-secure">🛡 {{ 'landing.demoReel.payment.secure' | translate }}</div>
        </div>
      </div>
    </ng-template>

    <ng-template #deliveryTpl let-scene="scene">
      <div class="s-delivery">
        <svg class="map" viewBox="0 0 380 200" preserveAspectRatio="xMidYMid slice">
          <rect width="380" height="200" fill="#E8F5E9"/>
          <path d="M0,140 Q80,110 160,130 T380,100" stroke="#A5D6A7" stroke-width="24" fill="none" opacity="0.6"/>
          <path d="M30,170 Q120,100 220,110 T360,40" stroke="#EF2B2D" stroke-width="3" fill="none" stroke-dasharray="6 4"/>
          <circle cx="30" cy="170" r="7" fill="#2E7D32"/>
          <circle cx="360" cy="40" r="8" fill="#EF2B2D"/>
          <g transform="translate(220, 110)">
            <circle r="9" fill="#FCD116"/>
            <text text-anchor="middle" dy="4" font-size="11" font-weight="700" fill="#1B1B1B">🛵</text>
          </g>
        </svg>
        <div class="track-rows">
          <div class="track-row done"><span class="t-dot"></span><b>{{ 'landing.demoReel.delivery.step1' | translate }}</b></div>
          <div class="track-row done"><span class="t-dot"></span><b>{{ 'landing.demoReel.delivery.step2' | translate }}</b></div>
          <div class="track-row active"><span class="t-dot pulse"></span><b>{{ 'landing.demoReel.delivery.step3' | translate }}</b></div>
          <div class="track-row"><span class="t-dot"></span>{{ 'landing.demoReel.delivery.step4' | translate }}</div>
        </div>
      </div>
    </ng-template>

    <ng-template #dashboardTpl let-scene="scene">
      <div class="s-dashboard">
        <div class="kpi-row">
          <div class="kpi"><span class="kpi-label">{{ 'landing.demoReel.dashboard.orders' | translate }}</span><span class="kpi-value">142</span><span class="kpi-trend up">+18%</span></div>
          <div class="kpi"><span class="kpi-label">{{ 'landing.demoReel.dashboard.revenue' | translate }}</span><span class="kpi-value">637k</span><span class="kpi-trend up">+9%</span></div>
          <div class="kpi"><span class="kpi-label">{{ 'landing.demoReel.dashboard.rating' | translate }}</span><span class="kpi-value">4.8★</span><span class="kpi-trend">+0.2</span></div>
        </div>
        <svg class="chart" viewBox="0 0 380 120" preserveAspectRatio="none">
          <defs>
            <linearGradient id="dashChart" x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stop-color="#009E49" stop-opacity="0.45"/>
              <stop offset="100%" stop-color="#009E49" stop-opacity="0"/>
            </linearGradient>
          </defs>
          <path d="M0,90 L40,75 L80,80 L120,55 L160,60 L200,40 L240,48 L280,30 L320,36 L380,18 L380,120 L0,120 Z" fill="url(#dashChart)"/>
          <path d="M0,90 L40,75 L80,80 L120,55 L160,60 L200,40 L240,48 L280,30 L320,36 L380,18" stroke="#009E49" stroke-width="2.5" fill="none"/>
        </svg>
        <div class="dash-rows">
          <div class="dash-row"><span class="dr-id">#A-4821</span><span>Cissé O.</span><span class="dr-status ok">{{ 'landing.demoReel.dashboard.delivered' | translate }}</span></div>
          <div class="dash-row"><span class="dr-id">#A-4822</span><span>Ouédraogo A.</span><span class="dr-status pending">{{ 'landing.demoReel.dashboard.inTransit' | translate }}</span></div>
        </div>
      </div>
    </ng-template>
  `,
  styles: [`
    .demo-reel {
      padding: clamp(48px, 8vw, 96px) 16px;
      background: linear-gradient(180deg, #FFFFFF 0%, #F5F7F1 100%);
    }
    .demo-inner {
      max-width: 1120px;
      margin: 0 auto;
      display: flex;
      flex-direction: column;
      gap: 28px;
    }
    .demo-head { text-align: center; max-width: 720px; margin: 0 auto; }
    .eyebrow {
      display: inline-block;
      font-size: 12px;
      font-weight: 700;
      color: #009E49;
      letter-spacing: 0.12em;
      text-transform: uppercase;
      margin-bottom: 8px;
    }
    .demo-head h2 {
      font-size: clamp(1.6rem, 3.4vw, 2.4rem);
      line-height: 1.15;
      color: #1B1B1B;
      margin: 0 0 10px;
      letter-spacing: -0.01em;
    }
    .demo-head .lead {
      color: #555;
      font-size: 1.05rem;
      line-height: 1.5;
    }

    .browser {
      position: relative;
      width: 100%;
      max-width: 980px;
      margin-inline: auto;
      background: #FFFFFF;
      border-radius: 14px;
      overflow: hidden;
      box-shadow:
        0 1px 2px rgba(0,0,0,0.04),
        0 24px 60px rgba(15, 62, 30, 0.18);
      isolation: isolate;
    }
    .browser-chrome {
      display: flex;
      align-items: center;
      gap: 12px;
      padding: 10px 14px;
      background: #F1F3F0;
      border-bottom: 1px solid #E0E3DE;
    }
    .dots { display: flex; gap: 6px; }
    .dot { width: 11px; height: 11px; border-radius: 50%; }
    .d-r { background: #FF5F57; } .d-y { background: #FEBC2E; } .d-g { background: #28C840; }
    .url {
      flex: 1;
      display: flex; align-items: center; gap: 8px;
      background: #FFFFFF;
      border: 1px solid #E0E3DE;
      border-radius: 6px;
      padding: 4px 10px;
      font: 500 12px system-ui, -apple-system, Segoe UI, Roboto, sans-serif;
      color: #444;
    }
    .url-text { color: #1B1B1B; font-weight: 500; }
    .lock { font-size: 10px; }
    .spacer { width: 42px; }

    .stage {
      position: relative;
      aspect-ratio: 16 / 9;
      background: #FAFBF7;
      overflow: hidden;
    }
    .scene {
      position: absolute;
      inset: 0;
      padding: 18px 22px;
      opacity: 0;
      transform: translateY(8px) scale(0.995);
      transition: opacity 600ms ease, transform 600ms ease;
      pointer-events: none;
    }
    .scene.active {
      opacity: 1;
      transform: translateY(0) scale(1);
      pointer-events: auto;
    }

    .progress {
      display: flex;
      gap: 6px;
      padding: 10px 14px;
      background: #FFFFFF;
      border-top: 1px solid #EDEFE9;
    }
    .prog-dot {
      flex: 1;
      height: 4px;
      background: #E4E7E0;
      border: 0;
      border-radius: 999px;
      padding: 0;
      cursor: pointer;
      position: relative;
      overflow: hidden;
      appearance: none;
    }
    .prog-dot .fill {
      display: block;
      width: 0;
      height: 100%;
      background: linear-gradient(90deg, #009E49, #FCD116);
      border-radius: inherit;
    }
    .prog-dot.active .fill {
      animation: prog 4000ms linear forwards;
    }
    @keyframes prog { from { width: 0%; } to { width: 100%; } }

    .caption {
      text-align: center;
      color: #444;
      font-size: 0.95rem;
      max-width: 680px;
      margin: 4px auto 0;
    }
    .caption strong { color: #1B1B1B; }
    .caption .sep { margin: 0 8px; opacity: 0.5; }

    /* ===== Scene: search ===== */
    .s-search { display: flex; flex-direction: column; gap: 14px; height: 100%; }
    .searchbar {
      display: flex; align-items: center; gap: 8px;
      background: #FFFFFF; border: 1px solid #E4E7E0;
      border-radius: 10px; padding: 8px 12px;
      font: 500 13px system-ui;
      color: #777;
    }
    .input-ghost { flex: 1; }
    .chip {
      padding: 3px 8px; border-radius: 999px;
      background: #EEF5E6; color: #2E7D32;
      font-size: 11px; font-weight: 600;
    }
    .chip-accent { background: #FFE9A8; color: #7A5A00; }
    .grid-cards {
      display: grid; grid-template-columns: repeat(3, 1fr);
      gap: 10px; flex: 1;
    }
    .mini-card {
      background: #FFFFFF; border-radius: 10px;
      border: 1px solid #EDEFE9;
      overflow: hidden; display: flex; flex-direction: column;
    }
    .mini-thumb { flex: 1; min-height: 60px; }
    .mini-body { padding: 8px 10px; }
    .mini-title { font-size: 12px; font-weight: 600; color: #1B1B1B; }
    .mini-meta {
      display: flex; justify-content: space-between; margin-top: 4px;
      font-size: 11px; color: #555;
    }
    .mini-price { font-weight: 700; color: #009E49; }
    .mini-badge { color: #7A5A00; font-weight: 600; }

    /* ===== Scene: listing ===== */
    .s-listing { display: grid; grid-template-rows: 58% 42%; height: 100%; gap: 12px; }
    .listing-hero {
      position: relative; border-radius: 10px;
      display: flex; align-items: flex-end; justify-content: flex-end;
      padding: 12px;
    }
    .hero-badges { position: absolute; top: 12px; left: 12px; display: flex; gap: 6px; flex-wrap: wrap; }
    .pill {
      padding: 3px 8px; border-radius: 999px; font-size: 10px; font-weight: 700;
      background: #FFFFFF; color: #1B1B1B; box-shadow: 0 1px 3px rgba(0,0,0,0.1);
    }
    .pill-halal { background: #009E49; color: #FFFFFF; }
    .pill-vet { background: #1976D2; color: #FFFFFF; }
    .hero-chick { font-size: 54px; filter: drop-shadow(0 4px 6px rgba(0,0,0,0.15)); }
    .listing-body { display: flex; flex-direction: column; gap: 6px; }
    .listing-title { font-size: 14px; font-weight: 700; color: #1B1B1B; }
    .listing-row { display: flex; gap: 12px; font-size: 11px; color: #555; flex-wrap: wrap; }
    .kv b { color: #1B1B1B; margin-right: 4px; }
    .listing-cta { display: flex; justify-content: space-between; align-items: center; margin-top: auto; }
    .price-big { font-size: 22px; font-weight: 800; color: #009E49; }
    .price-big small { font-size: 12px; color: #555; font-weight: 600; margin-left: 2px; }
    .cta-btn {
      background: #EF2B2D; color: #FFFFFF; padding: 7px 14px;
      border-radius: 8px; font-size: 12px; font-weight: 700;
    }

    /* ===== Scene: payment ===== */
    .s-payment { display: grid; grid-template-columns: 3fr 2fr; gap: 14px; height: 100%; }
    .pay-col { display: flex; flex-direction: column; gap: 8px; }
    .pay-title { font-size: 13px; font-weight: 700; color: #1B1B1B; margin-bottom: 4px; }
    .pay-method {
      display: flex; align-items: center; gap: 10px;
      padding: 8px 10px; border-radius: 8px;
      background: #FFFFFF; border: 1px solid #E4E7E0;
      font-size: 12px; font-weight: 500;
    }
    .pay-method.active { border-color: #009E49; background: #F0F9EE; }
    .pay-method .check { margin-left: auto; color: #009E49; font-weight: 800; }
    .pm-logo {
      width: 26px; height: 26px; border-radius: 6px;
      display: grid; place-items: center;
      font-size: 10px; font-weight: 800; color: #FFFFFF;
    }
    .pm-om { background: #FF6F00; }
    .pm-mm { background: #1976D2; }
    .pm-card { background: #5D4037; font-size: 14px; }
    .receipt {
      background: #FFFFFF; border: 1px solid #E4E7E0;
      border-radius: 10px; padding: 12px; gap: 6px;
    }
    .rcp-row { display: flex; justify-content: space-between; font-size: 12px; color: #555; }
    .rcp-row.total { font-size: 14px; color: #1B1B1B; font-weight: 800; padding-top: 6px; border-top: 1px dashed #E4E7E0; margin-top: 2px; }
    .pay-secure {
      margin-top: auto; text-align: center; font-size: 11px;
      padding: 6px; background: #F0F9EE; color: #2E7D32;
      border-radius: 6px; font-weight: 600;
    }

    /* ===== Scene: delivery ===== */
    .s-delivery { display: grid; grid-template-columns: 1.4fr 1fr; gap: 12px; height: 100%; }
    .map { width: 100%; height: 100%; border-radius: 10px; }
    .track-rows { display: flex; flex-direction: column; gap: 10px; justify-content: center; }
    .track-row { display: flex; align-items: center; gap: 10px; font-size: 12px; color: #888; }
    .track-row.done { color: #2E7D32; }
    .track-row.active { color: #1B1B1B; font-weight: 700; }
    .t-dot { width: 10px; height: 10px; border-radius: 50%; background: #D0D5C8; flex-shrink: 0; }
    .track-row.done .t-dot { background: #2E7D32; }
    .track-row.active .t-dot { background: #EF2B2D; }
    .t-dot.pulse { box-shadow: 0 0 0 0 rgba(239,43,45,0.6); animation: pulse-ring 1.6s ease-out infinite; }
    @keyframes pulse-ring {
      0%   { box-shadow: 0 0 0 0 rgba(239,43,45,0.6); }
      70%  { box-shadow: 0 0 0 10px rgba(239,43,45,0); }
      100% { box-shadow: 0 0 0 0 rgba(239,43,45,0); }
    }

    /* ===== Scene: dashboard ===== */
    .s-dashboard { display: flex; flex-direction: column; gap: 12px; height: 100%; }
    .kpi-row { display: grid; grid-template-columns: repeat(3, 1fr); gap: 10px; }
    .kpi {
      background: #FFFFFF; border: 1px solid #EDEFE9;
      border-radius: 10px; padding: 10px 12px;
      display: flex; flex-direction: column; gap: 2px;
    }
    .kpi-label { font-size: 11px; color: #666; text-transform: uppercase; letter-spacing: 0.06em; }
    .kpi-value { font-size: 20px; font-weight: 800; color: #1B1B1B; }
    .kpi-trend { font-size: 11px; font-weight: 700; color: #888; }
    .kpi-trend.up { color: #009E49; }
    .chart {
      height: 100px;
      width: 100%;
      background: #FFFFFF; border: 1px solid #EDEFE9;
      border-radius: 10px;
    }
    .dash-rows { display: flex; flex-direction: column; gap: 6px; }
    .dash-row {
      display: grid; grid-template-columns: 72px 1fr auto;
      align-items: center; gap: 10px;
      padding: 6px 10px;
      background: #FFFFFF; border: 1px solid #EDEFE9;
      border-radius: 8px; font-size: 12px;
    }
    .dr-id { color: #888; font-weight: 600; font-variant-numeric: tabular-nums; }
    .dr-status { font-size: 11px; font-weight: 700; padding: 2px 8px; border-radius: 999px; }
    .dr-status.ok { background: #EEF7E8; color: #2E7D32; }
    .dr-status.pending { background: #FFF4D4; color: #7A5A00; }

    @media (max-width: 720px) {
      .grid-cards { grid-template-columns: repeat(2, 1fr); }
      .s-payment { grid-template-columns: 1fr; }
      .s-delivery { grid-template-columns: 1fr; }
      .kpi-value { font-size: 16px; }
    }

    @media (prefers-reduced-motion: reduce) {
      .scene { transition: none; }
      .prog-dot.active .fill { animation: none; width: 100%; }
      .t-dot.pulse { animation: none; }
    }
  `],
})
export class DemoReelComponent implements OnInit {
  private readonly platformId = inject(PLATFORM_ID);
  private readonly zone = inject(NgZone);
  private readonly destroyRef = inject(DestroyRef);

  readonly scenes: (Scene & { urlPath: string })[] = [
    { id: 'search',    iconPath: '', titleKey: 'landing.demoReel.scenes.search.title',    captionKey: 'landing.demoReel.scenes.search.caption',    accent: '#009E49', urlPath: '/marketplace' },
    { id: 'listing',   iconPath: '', titleKey: 'landing.demoReel.scenes.listing.title',   captionKey: 'landing.demoReel.scenes.listing.caption',   accent: '#FCD116', urlPath: '/annonces/a-4812' },
    { id: 'payment',   iconPath: '', titleKey: 'landing.demoReel.scenes.payment.title',   captionKey: 'landing.demoReel.scenes.payment.caption',   accent: '#EF2B2D', urlPath: '/checkout' },
    { id: 'delivery',  iconPath: '', titleKey: 'landing.demoReel.scenes.delivery.title',  captionKey: 'landing.demoReel.scenes.delivery.caption',  accent: '#1976D2', urlPath: '/livraison/suivi' },
    { id: 'dashboard', iconPath: '', titleKey: 'landing.demoReel.scenes.dashboard.title', captionKey: 'landing.demoReel.scenes.dashboard.caption', accent: '#5D4037', urlPath: '/eleveur/tableau' },
  ];

  readonly searchCards = [
    { id: 1, titleKey: 'landing.demoReel.search.cardSasso',  price: '4 500 FCFA', rating: '4.8', bg: 'linear-gradient(135deg, #FFE9A8, #FCD116)' },
    { id: 2, titleKey: 'landing.demoReel.search.cardLocal',  price: '3 800 FCFA', rating: '4.6', bg: 'linear-gradient(135deg, #FFFFFF, #E8E0A8)' },
    { id: 3, titleKey: 'landing.demoReel.search.cardKoki',   price: '5 200 FCFA', rating: '4.9', bg: 'linear-gradient(135deg, #8D6E63, #D7CCC8)' },
    { id: 4, titleKey: 'landing.demoReel.search.cardFermier',price: '4 200 FCFA', rating: '4.7', bg: 'linear-gradient(135deg, #F5F5F5, #E0E0E0)' },
    { id: 5, titleKey: 'landing.demoReel.search.cardBio',    price: '6 000 FCFA', rating: '5.0', bg: 'linear-gradient(135deg, #C8E6C9, #81C784)' },
    { id: 6, titleKey: 'landing.demoReel.search.cardPintade',price: '7 500 FCFA', rating: '4.8', bg: 'linear-gradient(135deg, #607D8B, #B0BEC5)' },
  ];

  readonly activeIndex = signal(0);
  readonly isPaused = signal(false);
  readonly currentScene = computed(() => this.scenes[this.activeIndex()]);

  private timerId: ReturnType<typeof setInterval> | null = null;
  private readonly intervalMs = 4000;

  ngOnInit(): void {
    if (!isPlatformBrowser(this.platformId)) return;
    const reduced = window.matchMedia?.('(prefers-reduced-motion: reduce)').matches;
    if (reduced) return;
    this.start();
    this.destroyRef.onDestroy(() => this.stop());
  }

  private start(): void {
    this.zone.runOutsideAngular(() => {
      this.timerId = setInterval(() => {
        this.zone.run(() => {
          if (this.isPaused()) return;
          this.activeIndex.update(i => (i + 1) % this.scenes.length);
        });
      }, this.intervalMs);
    });
  }

  private stop(): void {
    if (this.timerId !== null) {
      clearInterval(this.timerId);
      this.timerId = null;
    }
  }

  pause(): void { this.isPaused.set(true); }
  resume(): void { this.isPaused.set(false); }

  jumpTo(i: number): void {
    this.activeIndex.set(i);
  }
}
