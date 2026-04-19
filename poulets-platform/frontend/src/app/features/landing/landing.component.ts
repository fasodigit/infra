import {
  Component,
  OnInit,
  OnDestroy,
  inject,
  signal,
  ChangeDetectionStrategy,
  ElementRef,
  AfterViewInit,
  PLATFORM_ID,
  NgZone,
} from '@angular/core';
import { isPlatformBrowser, CommonModule, ViewportScroller } from '@angular/common';
import { RouterLink } from '@angular/router';
import { MatButtonModule } from '@angular/material/button';
import { MatIconModule } from '@angular/material/icon';
import { MatToolbarModule } from '@angular/material/toolbar';
import { TranslateModule, TranslateService } from '@ngx-translate/core';
import { PublicDashboardComponent } from './public-dashboard.component';
import { HeroAnimationComponent } from './sections/hero-animation.component';
import { DemoReelComponent } from './sections/demo-reel.component';

@Component({
  selector: 'app-landing',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [
    CommonModule,
    RouterLink,
    MatButtonModule,
    MatIconModule,
    MatToolbarModule,
    TranslateModule,
    PublicDashboardComponent,
    HeroAnimationComponent,
    DemoReelComponent,
  ],
  template: `
    <!-- ===== NAVBAR ===== -->
    <nav class="landing-nav" [class.nav-scrolled]="navScrolled()">
      <div class="nav-inner">
        <a class="nav-logo" (click)="scrollTo('accueil')">
          <span class="logo-icon">
            <mat-icon>storefront</mat-icon>
          </span>
          <span class="logo-text">Poulets BF</span>
        </a>

        <ul class="nav-links" [class.nav-open]="mobileMenuOpen()">
          <li><a (click)="scrollTo('accueil'); closeMobile()">{{ 'landing.nav.accueil' | translate }}</a></li>
          <li><a (click)="scrollTo('presentation'); closeMobile()">{{ 'landing.nav.presentation' | translate }}</a></li>
          <li><a (click)="scrollTo('fonctionnalites'); closeMobile()">{{ 'landing.nav.fonctionnalites' | translate }}</a></li>
          <li><a (click)="scrollTo('securites'); closeMobile()">{{ 'landing.nav.securites' | translate }}</a></li>
          <li><a (click)="scrollTo('apropos'); closeMobile()">{{ 'landing.nav.apropos' | translate }}</a></li>
          <li class="nav-lang-mobile">
            <button class="lang-btn" [class.active]="currentLang() === 'fr'" (click)="switchLang('fr')">FR</button>
            <span class="lang-sep">|</span>
            <button class="lang-btn" [class.active]="currentLang() === 'en'" (click)="switchLang('en')">EN</button>
          </li>
          <li class="nav-cta-mobile">
            <a routerLink="/auth/login" class="btn-cta-nav" (click)="closeMobile()">{{ 'landing.nav.connexion' | translate }}</a>
          </li>
        </ul>

        <div class="nav-right">
          <div class="lang-switcher">
            <button class="lang-btn" [class.active]="currentLang() === 'fr'" (click)="switchLang('fr')">FR</button>
            <span class="lang-sep">|</span>
            <button class="lang-btn" [class.active]="currentLang() === 'en'" (click)="switchLang('en')">EN</button>
          </div>
          <a routerLink="/auth/login" class="btn-cta-nav">{{ 'landing.nav.connexion' | translate }}</a>
        </div>

        <button class="hamburger" (click)="toggleMobile()" [attr.aria-label]="'Menu'">
          <mat-icon>{{ mobileMenuOpen() ? 'close' : 'menu' }}</mat-icon>
        </button>
      </div>
    </nav>

    <!-- ===== HERO SECTION ===== -->
    <section id="accueil" class="hero-section" [class.has-video]="heroVideoReady()">
      <div class="hero-bg-shapes">
        <div class="shape shape-1"></div>
        <div class="shape shape-2"></div>
        <div class="shape shape-3"></div>
      </div>

      @if (heroVideoEnabled()) {
        <video
          class="hero-video"
          autoplay
          muted
          loop
          playsinline
          preload="metadata"
          aria-hidden="true"
          [class.hero-video-visible]="heroVideoReady()"
          (canplay)="onHeroVideoReady()"
          (error)="onHeroVideoError()"
        >
          <source src="assets/video/hero-farm.webm" type="video/webm">
          <source src="assets/video/hero-farm.mp4" type="video/mp4">
        </video>
      }
      <div class="hero-video-overlay" aria-hidden="true"></div>
      <div class="hero-atmosphere" aria-hidden="true"></div>

      <div class="hero-content">
        <div class="hero-text">
          <h1 class="hero-title">{{ 'landing.hero.title' | translate }}</h1>
          <p class="hero-subtitle">{{ 'landing.hero.subtitle' | translate }}</p>
          <div class="hero-ctas">
            <a routerLink="/auth/register" class="btn-primary">
              <mat-icon>agriculture</mat-icon>
              {{ 'landing.hero.cta_eleveur' | translate }}
            </a>
            <a routerLink="/auth/register" class="btn-outline">
              <mat-icon>shopping_cart</mat-icon>
              {{ 'landing.hero.cta_acheteur' | translate }}
            </a>
          </div>
        </div>
        <div class="hero-illustration">
          <app-hero-animation></app-hero-animation>
        </div>
      </div>
      <div class="hero-stats" #statsBar>
        <div class="stat-item" [class.animate-stat]="statsVisible()">
          <span class="stat-number">500+</span>
          <span class="stat-label">{{ 'landing.hero.stat_eleveurs' | translate }}</span>
        </div>
        <div class="stat-item" [class.animate-stat]="statsVisible()">
          <span class="stat-number">2 000+</span>
          <span class="stat-label">{{ 'landing.hero.stat_clients' | translate }}</span>
        </div>
        <div class="stat-item" [class.animate-stat]="statsVisible()">
          <span class="stat-number">50 000+</span>
          <span class="stat-label">{{ 'landing.hero.stat_transactions' | translate }}</span>
        </div>
        <div class="stat-item" [class.animate-stat]="statsVisible()">
          <span class="stat-number">13</span>
          <span class="stat-label">{{ 'landing.hero.stat_regions' | translate }}</span>
        </div>
      </div>
    </section>

    <!-- ===== PUBLIC DASHBOARD ===== -->
    <app-public-dashboard></app-public-dashboard>

    <!-- ===== PRESENTATION SECTION ===== -->
    <section id="presentation" class="section section-light">
      <div class="section-container">
        <h2 class="section-title">{{ 'landing.presentation.title' | translate }}</h2>
        <p class="section-subtitle">{{ 'landing.presentation.subtitle' | translate }}</p>
        <div class="steps-grid">
          @for (step of steps; track step.num; let i = $index) {
            <div class="step-card fade-in-card" [class.visible]="cardsVisible()[i] || false">
              <div class="step-number">{{ step.num }}</div>
              <div class="step-icon">
                <mat-icon>{{ step.icon }}</mat-icon>
              </div>
              <h3>{{ step.titleKey | translate }}</h3>
              <p>{{ step.descKey | translate }}</p>
            </div>
          }
        </div>
      </div>
    </section>

    <!-- ===== AVICULTURE REINVENTEE (glass pillars) ===== -->
    <section id="aviculture-reinventee" class="reinvented-section">
      <div class="reinvented-bg" aria-hidden="true">
        <span class="blob blob-1"></span>
        <span class="blob blob-2"></span>
        <span class="blob blob-3"></span>
      </div>
      <div class="reinvented-container">
        <header class="reinvented-head">
          <span class="reinvented-eyebrow">{{ 'landing.reinvented.eyebrow' | translate }}</span>
          <h2 class="reinvented-title">{{ 'landing.reinvented.title' | translate }}</h2>
          <p class="reinvented-lead">{{ 'landing.reinvented.lead' | translate }}</p>
        </header>
        <div class="reinvented-grid">
          @for (card of reinventedCards; track card.id) {
            <article class="reinvented-card">
              <div class="reinvented-icon" aria-hidden="true">
                <mat-icon>{{ card.icon }}</mat-icon>
              </div>
              <h3 class="reinvented-card-title">{{ card.titleKey | translate }}</h3>
              <p class="reinvented-card-desc">{{ card.descKey | translate }}</p>
              <div class="reinvented-pills">
                @for (pill of card.pills; track pill) {
                  <span class="reinvented-pill">{{ pill | translate }}</span>
                }
              </div>
            </article>
          }
        </div>
      </div>
    </section>

    <!-- ===== DEMO REEL SECTION ===== -->
    <app-demo-reel></app-demo-reel>

    <!-- ===== FONCTIONNALITES SECTION ===== -->
    <section id="fonctionnalites" class="section section-alt">
      <div class="section-container">
        <h2 class="section-title">{{ 'landing.features.title' | translate }}</h2>
        <p class="section-subtitle">{{ 'landing.features.subtitle' | translate }}</p>
        <div class="features-grid">
          @for (feat of features; track feat.icon; let i = $index) {
            <div class="feature-card fade-in-card" [class.visible]="featuresVisible()[i] || false">
              <div class="feature-icon-wrap">
                <mat-icon>{{ feat.icon }}</mat-icon>
              </div>
              <h3>{{ feat.titleKey | translate }}</h3>
              <p>{{ feat.descKey | translate }}</p>
            </div>
          }
        </div>
      </div>
    </section>

    <!-- ===== SECURITES SECTION ===== -->
    <section id="securites" class="section section-dark">
      <div class="section-container">
        <h2 class="section-title light">{{ 'landing.security.title' | translate }}</h2>
        <p class="section-subtitle light">{{ 'landing.security.subtitle' | translate }}</p>
        <div class="security-grid">
          @for (sec of securityItems; track sec.icon; let i = $index) {
            <div class="security-card fade-in-card" [class.visible]="securityVisible()[i] || false">
              <div class="security-icon-wrap">
                <mat-icon>{{ sec.icon }}</mat-icon>
              </div>
              <h3>{{ sec.titleKey | translate }}</h3>
              <p>{{ sec.descKey | translate }}</p>
            </div>
          }
        </div>
      </div>
    </section>

    <!-- ===== A PROPOS SECTION ===== -->
    <section id="apropos" class="section section-light">
      <div class="section-container">
        <h2 class="section-title">{{ 'landing.about.title' | translate }}</h2>
        <p class="about-description">{{ 'landing.about.description' | translate }}</p>
        <div class="platforms-grid">
          @for (plat of platforms; track plat.icon) {
            <div class="platform-badge">
              <div class="platform-icon">
                <mat-icon>{{ plat.icon }}</mat-icon>
              </div>
              <span class="platform-name">{{ plat.labelKey | translate }}</span>
            </div>
          }
        </div>
        <div class="contact-block">
          <mat-icon>email</mat-icon>
          <span>{{ 'landing.about.contact' | translate }}</span>
        </div>
      </div>
    </section>

    <!-- ===== FOOTER ===== -->
    <footer class="landing-footer">
      <div class="footer-inner">
        <div class="footer-links">
          <a href="#">{{ 'landing.footer.terms' | translate }}</a>
          <a href="#">{{ 'landing.footer.privacy' | translate }}</a>
          <a href="#">{{ 'landing.footer.help' | translate }}</a>
        </div>
        <div class="footer-social">
          <a href="#" aria-label="Facebook"><mat-icon>facebook</mat-icon></a>
          <a href="#" aria-label="Twitter"><mat-icon>tag</mat-icon></a>
          <a href="#" aria-label="LinkedIn"><mat-icon>work</mat-icon></a>
        </div>
        <p class="footer-copy">{{ 'landing.footer.copyright' | translate }}</p>
        <p class="footer-powered">{{ 'landing.footer.powered' | translate }}</p>
      </div>
    </footer>
  `,
  styles: [`
    /* ===================================================================
       RESET & HOST
       =================================================================== */
    :host {
      display: block;
      overflow-x: hidden;
      --green: #009639;
      --green-dark: #006B28;
      --red: #EF2B2D;
      --gold: #FCD116;
      --dark: #1B3A5C;
      --dark-deep: #0F2440;
      --bg-white: #FFFFFF;
      --bg-alt: #F8FAFC;
      --text: #1E293B;
      --text-muted: #64748B;
      --radius: 16px;
      --radius-sm: 8px;
      --shadow: 0 4px 24px rgba(0, 0, 0, 0.08);
      --shadow-lg: 0 8px 40px rgba(0, 0, 0, 0.12);
      --transition: 0.3s cubic-bezier(0.4, 0, 0.2, 1);
    }

    * {
      box-sizing: border-box;
    }

    /* ===================================================================
       NAVBAR
       =================================================================== */
    .landing-nav {
      position: fixed;
      top: 16px;
      left: 50%;
      transform: translateX(-50%);
      z-index: 1000;
      width: calc(100% - 32px);
      max-width: 1180px;
      height: 62px;
      padding: 0 8px 0 20px;
      border-radius: 999px;
      background: rgba(20, 20, 20, 0.22);
      backdrop-filter: blur(18px) saturate(150%);
      -webkit-backdrop-filter: blur(18px) saturate(150%);
      border: 1px solid rgba(255, 255, 255, 0.16);
      box-shadow:
        0 8px 32px rgba(0, 0, 0, 0.18),
        inset 0 1px 0 rgba(255, 255, 255, 0.12);
      transition:
        background 280ms ease,
        border-color 280ms ease,
        box-shadow 280ms ease,
        top 280ms ease;
    }

    .landing-nav.nav-scrolled {
      background: rgba(255, 255, 255, 0.78);
      border-color: rgba(0, 0, 0, 0.06);
      box-shadow:
        0 12px 40px rgba(0, 0, 0, 0.08),
        inset 0 1px 0 rgba(255, 255, 255, 0.9);
    }

    .nav-inner {
      max-width: 100%;
      margin: 0 auto;
      display: flex;
      align-items: center;
      height: 100%;
      gap: 16px;
    }

    .nav-logo {
      display: flex;
      align-items: center;
      gap: 10px;
      text-decoration: none;
      cursor: pointer;
      flex-shrink: 0;
    }

    .logo-icon {
      display: flex;
      align-items: center;
      justify-content: center;
      width: 40px;
      height: 40px;
      border-radius: 10px;
      background: var(--green);
      color: white;
    }

    .logo-icon mat-icon {
      font-size: 22px;
      width: 22px;
      height: 22px;
    }

    .logo-text {
      font-size: 1.35rem;
      font-weight: 700;
      color: white;
      letter-spacing: -0.3px;
    }

    .nav-scrolled .logo-text {
      color: var(--dark);
    }

    .nav-links {
      display: flex;
      align-items: center;
      gap: 8px;
      list-style: none;
      margin: 0 auto;
      padding: 0;
    }

    .nav-links li a {
      padding: 8px 16px;
      border-radius: 8px;
      font-size: 0.9rem;
      font-weight: 500;
      color: rgba(255, 255, 255, 0.9);
      text-decoration: none;
      cursor: pointer;
      transition: background var(--transition), color var(--transition);
      white-space: nowrap;
    }

    .nav-links li a:hover {
      background: rgba(255, 255, 255, 0.15);
    }

    .nav-scrolled .nav-links li a {
      color: var(--text);
    }

    .nav-scrolled .nav-links li a:hover {
      background: rgba(0, 150, 57, 0.08);
      color: var(--green);
    }

    .nav-lang-mobile,
    .nav-cta-mobile {
      display: none;
    }

    .nav-right {
      display: flex;
      align-items: center;
      gap: 16px;
      flex-shrink: 0;
    }

    .lang-switcher {
      display: flex;
      align-items: center;
      gap: 4px;
    }

    .lang-btn {
      background: none;
      border: none;
      padding: 4px 8px;
      font-size: 0.82rem;
      font-weight: 600;
      cursor: pointer;
      border-radius: 4px;
      transition: all var(--transition);
      color: rgba(255, 255, 255, 0.75);
    }

    .lang-btn.active {
      color: white;
      background: rgba(255, 255, 255, 0.2);
    }

    .nav-scrolled .lang-btn {
      color: var(--text-muted);
    }

    .nav-scrolled .lang-btn.active {
      color: var(--green);
      background: rgba(0, 150, 57, 0.1);
    }

    .lang-sep {
      color: rgba(255, 255, 255, 0.4);
      font-size: 0.8rem;
    }

    .nav-scrolled .lang-sep {
      color: var(--text-muted);
    }

    .btn-cta-nav {
      display: inline-flex;
      align-items: center;
      padding: 10px 24px;
      border-radius: 50px;
      font-size: 0.9rem;
      font-weight: 600;
      text-decoration: none;
      transition: all var(--transition);
      background: white;
      color: var(--green);
      box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
    }

    .btn-cta-nav:hover {
      transform: translateY(-1px);
      box-shadow: 0 4px 16px rgba(0, 0, 0, 0.15);
    }

    .nav-scrolled .btn-cta-nav {
      background: var(--green);
      color: white;
    }

    .hamburger {
      display: none;
      background: none;
      border: none;
      cursor: pointer;
      color: white;
      padding: 4px;
    }

    .nav-scrolled .hamburger {
      color: var(--dark);
    }

    /* Mobile nav */
    @media (max-width: 900px) {
      .landing-nav {
        padding: 0 6px 0 16px;
      }
      .logo-text {
        font-size: 1.1rem;
      }
      .nav-links {
        position: fixed;
        top: 88px;
        left: 12px;
        right: 12px;
        max-height: calc(100dvh - 104px);
        background: #FFFFFF;
        border: 1px solid rgba(0, 0, 0, 0.06);
        border-radius: 22px;
        box-shadow: 0 20px 60px rgba(0, 0, 0, 0.22);
        flex-direction: column;
        align-items: stretch;
        justify-content: flex-start;
        padding: 14px;
        gap: 2px;
        transform: translateX(120%);
        transition: transform 0.35s cubic-bezier(0.4, 0, 0.2, 1);
        overflow-y: auto;
        -webkit-overflow-scrolling: touch;
      }

      .nav-links.nav-open {
        transform: translateX(0);
      }

      .nav-links li a {
        color: var(--text) !important;
        font-size: 1.05rem;
        padding: 14px 16px;
        border-radius: 12px;
      }

      .nav-links li a:hover {
        background: rgba(0, 150, 57, 0.08) !important;
        color: var(--green) !important;
      }

      .nav-lang-mobile {
        display: flex !important;
        align-items: center;
        gap: 8px;
        padding: 14px 16px;
      }

      .nav-lang-mobile .lang-btn {
        color: var(--text-muted);
        font-size: 0.95rem;
        padding: 6px 12px;
      }

      .nav-lang-mobile .lang-btn.active {
        color: var(--green);
        background: rgba(0, 150, 57, 0.1);
      }

      .nav-lang-mobile .lang-sep {
        color: var(--text-muted);
      }

      .nav-cta-mobile {
        display: block !important;
        padding: 14px 16px;
      }

      .nav-cta-mobile .btn-cta-nav {
        display: block;
        text-align: center;
        background: var(--green);
        color: white;
        padding: 14px 24px;
        font-size: 1rem;
      }

      .nav-right {
        display: none;
      }

      .hamburger {
        display: block;
        margin-left: auto;
      }
    }

    /* ===================================================================
       HERO SECTION
       =================================================================== */
    .hero-section {
      position: relative;
      min-height: 100vh;
      display: flex;
      flex-direction: column;
      justify-content: center;
      background: linear-gradient(135deg, var(--green-dark) 0%, var(--green) 40%, #2D8F3E 70%, #5CAB3C 100%);
      overflow: hidden;
      padding-top: 96px;
    }

    /* ==== Hero background video (cinematic) ==== */
    .hero-video {
      position: absolute;
      inset: 0;
      width: 100%;
      height: 100%;
      object-fit: cover;
      z-index: 0;
      opacity: 0;
      transition: opacity 1400ms ease-out;
      pointer-events: none;
    }
    .hero-video.hero-video-visible {
      opacity: 1;
    }

    .hero-video-overlay {
      position: absolute;
      inset: 0;
      z-index: 0;
      pointer-events: none;
      background:
        linear-gradient(180deg,
          rgba(10, 30, 15, 0.35) 0%,
          rgba(10, 30, 15, 0.50) 55%,
          rgba(15, 62, 30, 0.78) 100%);
    }

    /* Atmosphère cinématique : radial sweep doré qui respire
       (visible en toutes circonstances, renforce l'ambiance golden-hour) */
    .hero-atmosphere {
      position: absolute;
      inset: 0;
      z-index: 0;
      pointer-events: none;
      background:
        radial-gradient(ellipse at 80% 20%, rgba(252, 209, 22, 0.16) 0%, transparent 55%),
        radial-gradient(ellipse at 15% 80%, rgba(252, 209, 22, 0.08) 0%, transparent 60%);
      animation: hero-atmosphere-drift 18s ease-in-out infinite alternate;
      mix-blend-mode: screen;
    }
    @keyframes hero-atmosphere-drift {
      from { transform: translate3d(0, 0, 0) scale(1); }
      to   { transform: translate3d(-1.5%, 1%, 0) scale(1.04); }
    }

    /* Assombrissement subtil des shapes quand une vidéo est active */
    .hero-section.has-video .hero-bg-shapes { opacity: 0.35; }

    .hero-bg-shapes {
      position: absolute;
      inset: 0;
      pointer-events: none;
      overflow: hidden;
      z-index: 0;
      transition: opacity 900ms ease;
    }

    .shape {
      position: absolute;
      border-radius: 50%;
      opacity: 0.08;
    }

    .shape-1 {
      width: 600px;
      height: 600px;
      background: var(--gold);
      top: -200px;
      right: -150px;
    }

    .shape-2 {
      width: 400px;
      height: 400px;
      background: white;
      bottom: -100px;
      left: -100px;
    }

    .shape-3 {
      width: 200px;
      height: 200px;
      background: var(--gold);
      top: 50%;
      left: 30%;
    }

    .hero-content {
      max-width: 1280px;
      width: 100%;
      margin: 0 auto;
      padding: 60px 32px 40px;
      display: grid;
      grid-template-columns: 1fr 1fr;
      align-items: center;
      gap: 48px;
      position: relative;
      z-index: 1;
    }

    .hero-text {
      color: white;
    }

    .hero-title {
      font-family: 'Fraunces', 'Playfair Display', Georgia, serif;
      font-style: italic;
      font-optical-sizing: auto;
      font-size: clamp(1.95rem, 5.5vw, 4.25rem);
      font-weight: 500;
      line-height: 1.08;
      letter-spacing: -0.015em;
      margin: 0 0 20px;
      color: #FFFFFF;
      text-shadow: 0 2px 24px rgba(0, 0, 0, 0.35);
    }

    .hero-subtitle {
      font-family: 'Roboto', system-ui, -apple-system, sans-serif;
      font-size: clamp(1rem, 1.4vw, 1.2rem);
      font-weight: 400;
      line-height: 1.55;
      margin: 0 0 36px;
      opacity: 0.88;
      max-width: 540px;
      letter-spacing: 0.005em;
    }

    .hero-ctas {
      display: flex;
      flex-wrap: wrap;
      gap: 16px;
    }

    .btn-primary,
    .btn-outline {
      display: inline-flex;
      align-items: center;
      gap: 8px;
      padding: 14px 32px;
      border-radius: 50px;
      font-size: 1rem;
      font-weight: 600;
      text-decoration: none;
      transition: all var(--transition);
      cursor: pointer;
    }

    .btn-primary {
      background: var(--gold);
      color: var(--dark);
      border: 2px solid var(--gold);
      box-shadow: 0 4px 20px rgba(252, 209, 22, 0.35);
    }

    .btn-primary:hover {
      transform: translateY(-2px);
      box-shadow: 0 6px 28px rgba(252, 209, 22, 0.45);
      background: #ffe04a;
      border-color: #ffe04a;
    }

    .btn-primary mat-icon,
    .btn-outline mat-icon {
      font-size: 20px;
      width: 20px;
      height: 20px;
    }

    .btn-outline {
      background: transparent;
      color: white;
      border: 2px solid rgba(255, 255, 255, 0.7);
    }

    .btn-outline:hover {
      background: rgba(255, 255, 255, 0.12);
      border-color: white;
      transform: translateY(-2px);
    }

    .hero-illustration {
      display: flex;
      align-items: center;
      justify-content: center;
    }

    .hero-svg {
      width: 100%;
      max-width: 420px;
      height: auto;
      filter: drop-shadow(0 8px 32px rgba(0, 0, 0, 0.2));
      border-radius: 16px;
    }

    /* ==== Glass stat cards (style Viktor Oddy / claude.ai design) ==== */
    .hero-stats {
      position: relative;
      z-index: 1;
      display: flex;
      justify-content: center;
      align-items: stretch;
      flex-wrap: wrap;
      gap: 14px;
      max-width: 1180px;
      margin: 0 auto;
      padding: 0 32px 48px;
    }

    .stat-item {
      flex: 1 1 220px;
      max-width: 280px;
      min-width: 180px;
      padding: 22px 26px;
      color: white;
      display: flex;
      flex-direction: column;
      gap: 4px;
      background: rgba(255, 255, 255, 0.08);
      backdrop-filter: blur(16px) saturate(140%);
      -webkit-backdrop-filter: blur(16px) saturate(140%);
      border: 1px solid rgba(255, 255, 255, 0.18);
      border-radius: 20px;
      box-shadow:
        0 10px 30px rgba(0, 0, 0, 0.18),
        inset 0 1px 0 rgba(255, 255, 255, 0.15);
      opacity: 0;
      transform: translateY(18px);
      transition:
        opacity 520ms cubic-bezier(0.2, 0.0, 0.2, 1),
        transform 520ms cubic-bezier(0.2, 0.0, 0.2, 1),
        border-color 280ms ease,
        box-shadow 280ms ease;
    }

    .stat-item:hover {
      transform: translateY(-3px);
      border-color: rgba(255, 255, 255, 0.32);
      box-shadow:
        0 16px 44px rgba(0, 0, 0, 0.22),
        inset 0 1px 0 rgba(255, 255, 255, 0.22);
    }

    .stat-item.animate-stat {
      opacity: 1;
      transform: translateY(0);
    }

    .stat-item:nth-child(2).animate-stat { transition-delay: 0.08s; }
    .stat-item:nth-child(3).animate-stat { transition-delay: 0.16s; }
    .stat-item:nth-child(4).animate-stat { transition-delay: 0.24s; }

    .stat-number {
      display: block;
      font-family: 'Fraunces', 'Playfair Display', Georgia, serif;
      font-style: italic;
      font-optical-sizing: auto;
      font-size: clamp(2rem, 3.2vw, 2.85rem);
      font-weight: 500;
      line-height: 1;
      letter-spacing: -0.02em;
      color: var(--gold, #FCD116);
    }

    .stat-label {
      display: block;
      font-family: 'Roboto', system-ui, sans-serif;
      font-size: 0.72rem;
      font-weight: 600;
      opacity: 0.88;
      text-transform: uppercase;
      letter-spacing: 0.14em;
      margin-top: 2px;
    }

    @media (max-width: 900px) {
      .hero-section {
        min-height: auto;
        padding-top: 88px;
      }
      .hero-content {
        grid-template-columns: 1fr;
        padding: 24px 20px 28px;
        gap: 22px;
        text-align: center;
      }

      .hero-subtitle {
        margin-left: auto;
        margin-right: auto;
      }

      .hero-ctas {
        justify-content: center;
      }

      .hero-illustration {
        order: -1;
        max-width: 320px;
        margin: 0 auto;
      }

      .hero-svg { max-width: 280px; }

      .hero-stats {
        gap: 10px;
        padding: 0 16px 32px;
      }
      .stat-item {
        flex: 1 1 calc(50% - 5px);
        min-width: 140px;
        padding: 16px 18px;
      }
      .stat-number { font-size: 1.85rem; }
      .stat-label { font-size: 0.66rem; letter-spacing: 0.12em; }

      .btn-primary, .btn-outline {
        padding: 12px 22px;
        font-size: 0.95rem;
      }
    }

    @media (max-width: 480px) {
      .hero-section { padding-top: 84px; }
      .hero-content { padding: 16px 16px 20px; gap: 18px; }
      .hero-title { line-height: 1.1; letter-spacing: -0.01em; }
      .hero-subtitle { font-size: 0.95rem; line-height: 1.5; margin-bottom: 26px; }
      .hero-illustration { max-width: 280px; }

      .hero-ctas { gap: 10px; width: 100%; }
      .btn-primary, .btn-outline {
        flex: 1 1 calc(50% - 5px);
        justify-content: center;
        padding: 11px 14px;
        font-size: 0.9rem;
        white-space: nowrap;
      }
      .btn-primary mat-icon, .btn-outline mat-icon {
        font-size: 17px; width: 17px; height: 17px;
      }

      .hero-stats {
        gap: 8px;
        padding: 0 12px 28px;
      }
      .stat-item {
        padding: 14px 14px;
        border-radius: 16px;
        flex: 1 1 calc(50% - 4px);
        min-width: 0;
      }
      .stat-number { font-size: 1.55rem; }
      .stat-label { font-size: 0.6rem; }
    }

    /* ===================================================================
       SECTIONS COMMON
       =================================================================== */
    .section {
      padding: 80px 24px;
    }

    .section-light {
      background: var(--bg-white);
    }

    .section-alt {
      background: var(--bg-alt);
    }

    .section-dark {
      background: linear-gradient(135deg, var(--dark) 0%, var(--dark-deep) 100%);
    }

    .section-container {
      max-width: 1200px;
      margin: 0 auto;
    }

    .section-title {
      text-align: center;
      font-size: 2.25rem;
      font-weight: 800;
      color: var(--text);
      margin: 0 0 12px;
      letter-spacing: -0.3px;
    }

    .section-title.light {
      color: white;
    }

    .section-subtitle {
      text-align: center;
      font-size: 1.1rem;
      color: var(--text-muted);
      margin: 0 0 56px;
      max-width: 600px;
      margin-left: auto;
      margin-right: auto;
      line-height: 1.6;
    }

    .section-subtitle.light {
      color: rgba(255, 255, 255, 0.75);
    }

    /* ===================================================================
       FADE IN CARDS (IntersectionObserver)
       =================================================================== */
    .fade-in-card {
      opacity: 0;
      transform: translateY(32px);
      transition: opacity 0.6s ease, transform 0.6s ease;
    }

    .fade-in-card.visible {
      opacity: 1;
      transform: translateY(0);
    }

    /* ===================================================================
       PRESENTATION / STEPS SECTION
       =================================================================== */
    .steps-grid {
      display: grid;
      grid-template-columns: repeat(3, 1fr);
      gap: 40px;
    }

    .step-card {
      text-align: center;
      padding: 40px 24px;
      position: relative;
    }

    .step-number {
      position: absolute;
      top: 0;
      left: 50%;
      transform: translateX(-50%);
      width: 48px;
      height: 48px;
      border-radius: 50%;
      background: linear-gradient(135deg, var(--green), #2D8F3E);
      color: white;
      display: flex;
      align-items: center;
      justify-content: center;
      font-size: 1.25rem;
      font-weight: 700;
      box-shadow: 0 4px 16px rgba(0, 150, 57, 0.3);
    }

    .step-icon {
      margin-top: 36px;
      margin-bottom: 16px;
    }

    .step-icon mat-icon {
      font-size: 48px;
      width: 48px;
      height: 48px;
      color: var(--green);
    }

    .step-card h3 {
      font-size: 1.2rem;
      font-weight: 700;
      margin: 0 0 12px;
      color: var(--text);
    }

    .step-card p {
      font-size: 0.95rem;
      color: var(--text-muted);
      line-height: 1.6;
      margin: 0;
    }

    @media (max-width: 768px) {
      .steps-grid {
        grid-template-columns: 1fr;
        gap: 48px;
      }
    }

    /* ===================================================================
       FEATURES SECTION
       =================================================================== */
    .features-grid {
      display: grid;
      grid-template-columns: repeat(3, 1fr);
      gap: 24px;
    }

    .feature-card {
      background: var(--bg-white);
      border-radius: var(--radius);
      padding: 32px 24px;
      box-shadow: var(--shadow);
      transition: transform var(--transition), box-shadow var(--transition);
      border: 1px solid rgba(0, 0, 0, 0.04);
    }

    .feature-card:hover {
      transform: translateY(-4px);
      box-shadow: var(--shadow-lg);
    }

    .feature-icon-wrap {
      width: 56px;
      height: 56px;
      border-radius: 14px;
      background: linear-gradient(135deg, rgba(0, 150, 57, 0.1), rgba(0, 150, 57, 0.05));
      display: flex;
      align-items: center;
      justify-content: center;
      margin-bottom: 20px;
    }

    .feature-icon-wrap mat-icon {
      font-size: 28px;
      width: 28px;
      height: 28px;
      color: var(--green);
    }

    .feature-card h3 {
      font-size: 1.1rem;
      font-weight: 700;
      margin: 0 0 10px;
      color: var(--text);
    }

    .feature-card p {
      font-size: 0.9rem;
      color: var(--text-muted);
      line-height: 1.6;
      margin: 0;
    }

    @media (max-width: 1024px) {
      .features-grid {
        grid-template-columns: repeat(2, 1fr);
      }
    }

    @media (max-width: 600px) {
      .features-grid {
        grid-template-columns: 1fr;
      }
    }

    /* Feature card staggered animations */
    .feature-card:nth-child(2).visible { transition-delay: 0.05s; }
    .feature-card:nth-child(3).visible { transition-delay: 0.1s; }
    .feature-card:nth-child(4).visible { transition-delay: 0.15s; }
    .feature-card:nth-child(5).visible { transition-delay: 0.2s; }
    .feature-card:nth-child(6).visible { transition-delay: 0.25s; }
    .feature-card:nth-child(7).visible { transition-delay: 0.3s; }
    .feature-card:nth-child(8).visible { transition-delay: 0.35s; }
    .feature-card:nth-child(9).visible { transition-delay: 0.4s; }

    /* ===================================================================
       SECURITY SECTION
       =================================================================== */
    .security-grid {
      display: grid;
      grid-template-columns: repeat(3, 1fr);
      gap: 24px;
    }

    .security-card {
      background: rgba(255, 255, 255, 0.06);
      border: 1px solid rgba(255, 255, 255, 0.1);
      border-radius: var(--radius);
      padding: 32px 24px;
      transition: transform var(--transition), background var(--transition);
    }

    .security-card:hover {
      transform: translateY(-4px);
      background: rgba(255, 255, 255, 0.1);
    }

    .security-icon-wrap {
      width: 52px;
      height: 52px;
      border-radius: 12px;
      background: linear-gradient(135deg, var(--green), #2D8F3E);
      display: flex;
      align-items: center;
      justify-content: center;
      margin-bottom: 20px;
      box-shadow: 0 4px 16px rgba(0, 150, 57, 0.25);
    }

    .security-icon-wrap mat-icon {
      font-size: 26px;
      width: 26px;
      height: 26px;
      color: white;
    }

    .security-card h3 {
      font-size: 1.05rem;
      font-weight: 700;
      color: white;
      margin: 0 0 10px;
    }

    .security-card p {
      font-size: 0.88rem;
      color: rgba(255, 255, 255, 0.7);
      line-height: 1.6;
      margin: 0;
    }

    /* Stagger security cards */
    .security-card:nth-child(2).visible { transition-delay: 0.05s; }
    .security-card:nth-child(3).visible { transition-delay: 0.1s; }
    .security-card:nth-child(4).visible { transition-delay: 0.15s; }
    .security-card:nth-child(5).visible { transition-delay: 0.2s; }
    .security-card:nth-child(6).visible { transition-delay: 0.25s; }

    @media (max-width: 1024px) {
      .security-grid {
        grid-template-columns: repeat(2, 1fr);
      }
    }

    @media (max-width: 600px) {
      .security-grid {
        grid-template-columns: 1fr;
      }
    }

    /* ===================================================================
       ABOUT SECTION
       =================================================================== */
    .about-description {
      text-align: center;
      font-size: 1.1rem;
      color: var(--text-muted);
      line-height: 1.7;
      max-width: 720px;
      margin: 0 auto 48px;
    }

    .platforms-grid {
      display: flex;
      flex-wrap: wrap;
      justify-content: center;
      gap: 20px;
      margin-bottom: 48px;
    }

    .platform-badge {
      display: flex;
      flex-direction: column;
      align-items: center;
      gap: 8px;
      padding: 20px 16px;
      border-radius: var(--radius);
      background: var(--bg-alt);
      border: 1px solid rgba(0, 0, 0, 0.06);
      min-width: 120px;
      transition: transform var(--transition), box-shadow var(--transition);
    }

    .platform-badge:hover {
      transform: translateY(-3px);
      box-shadow: var(--shadow);
    }

    .platform-icon {
      width: 48px;
      height: 48px;
      border-radius: 12px;
      background: linear-gradient(135deg, var(--green), #2D8F3E);
      display: flex;
      align-items: center;
      justify-content: center;
    }

    .platform-icon mat-icon {
      font-size: 24px;
      width: 24px;
      height: 24px;
      color: white;
    }

    .platform-name {
      font-size: 0.82rem;
      font-weight: 600;
      color: var(--text);
      text-align: center;
    }

    .contact-block {
      display: flex;
      align-items: center;
      justify-content: center;
      gap: 10px;
      padding: 16px;
      color: var(--text-muted);
      font-size: 0.95rem;
    }

    .contact-block mat-icon {
      color: var(--green);
    }

    @media (max-width: 600px) {
      .platforms-grid {
        gap: 12px;
      }

      .platform-badge {
        min-width: 100px;
        padding: 16px 12px;
      }
    }

    /* ===================================================================
       FOOTER
       =================================================================== */
    .landing-footer {
      background: var(--dark-deep);
      color: rgba(255, 255, 255, 0.8);
      padding: 48px 24px 32px;
    }

    .footer-inner {
      max-width: 1200px;
      margin: 0 auto;
      text-align: center;
    }

    .footer-links {
      display: flex;
      justify-content: center;
      flex-wrap: wrap;
      gap: 24px;
      margin-bottom: 24px;
    }

    .footer-links a {
      color: rgba(255, 255, 255, 0.7);
      text-decoration: none;
      font-size: 0.9rem;
      transition: color var(--transition);
    }

    .footer-links a:hover {
      color: var(--gold);
    }

    .footer-social {
      display: flex;
      justify-content: center;
      gap: 16px;
      margin-bottom: 24px;
    }

    .footer-social a {
      display: flex;
      align-items: center;
      justify-content: center;
      width: 40px;
      height: 40px;
      border-radius: 50%;
      background: rgba(255, 255, 255, 0.08);
      color: rgba(255, 255, 255, 0.7);
      text-decoration: none;
      transition: all var(--transition);
    }

    .footer-social a:hover {
      background: var(--green);
      color: white;
      transform: translateY(-2px);
    }

    .footer-social mat-icon {
      font-size: 20px;
      width: 20px;
      height: 20px;
    }

    .footer-copy {
      font-size: 0.85rem;
      margin: 0 0 6px;
      opacity: 0.8;
    }

    .footer-powered {
      font-size: 0.78rem;
      margin: 0;
      opacity: 0.5;
      letter-spacing: 0.5px;
    }

    /* ===================================================================
       GLOBAL RESPONSIVE TWEAKS
       =================================================================== */
    @media (max-width: 900px) {
      .reinvented-section {
        padding: clamp(56px, 10vw, 80px) 20px;
      }
      .reinvented-head {
        margin-bottom: 36px;
      }
      .reinvented-grid {
        gap: 14px;
      }
      .reinvented-card {
        padding: 26px 22px 22px;
      }
      .reinvented-card-title {
        font-size: 1.35rem;
      }
      .reinvented-card-desc {
        font-size: 0.9rem;
      }
    }

    @media (max-width: 600px) {
      .section {
        padding: 52px 16px;
      }
      .section-title {
        font-size: 1.75rem;
      }
      .section-subtitle {
        font-size: 1rem;
        margin-bottom: 36px;
      }
      .reinvented-section {
        padding: 56px 16px 48px;
      }
      .reinvented-title {
        font-size: 1.9rem;
      }
      .reinvented-lead {
        font-size: 0.95rem;
      }
      .reinvented-pill {
        font-size: 0.68rem;
        padding: 3px 8px;
      }
    }

    @media (max-width: 768px) {
      .step-card {
        transition-delay: 0s !important;
      }

      .feature-card {
        transition-delay: 0s !important;
      }

      .security-card {
        transition-delay: 0s !important;
      }
    }

    /* =================================================================
       ANIMATIONS HERO & MICRO-INTERACTIONS (ajoutées 2026-04)
       ================================================================= */

    @keyframes faso-hero-rise {
      from { opacity: 0; transform: translateY(32px); }
      to   { opacity: 1; transform: translateY(0); }
    }
    @keyframes faso-hero-pop {
      0%   { opacity: 0; transform: scale(0.88); }
      60%  { opacity: 1; transform: scale(1.03); }
      100% { transform: scale(1); }
    }
    @keyframes faso-float-subtle {
      0%, 100% { transform: translateY(0) rotate(0deg); }
      50%      { transform: translateY(-6px) rotate(1deg); }
    }
    @keyframes faso-float-bigger {
      0%, 100% { transform: translateY(0) rotate(0deg); }
      50%      { transform: translateY(-10px) rotate(-1.5deg); }
    }
    @keyframes faso-shine {
      0%, 100% { opacity: 0.9; }
      50%      { opacity: 1; filter: brightness(1.15); }
    }
    @keyframes faso-shape-drift {
      0%, 100% { transform: translate(0, 0) scale(1); }
      33%      { transform: translate(20px, -30px) scale(1.05); }
      66%      { transform: translate(-15px, 20px) scale(0.95); }
    }
    @keyframes faso-fade-in-basic {
      from { opacity: 0; }
      to   { opacity: 1; }
    }

    /* Hero : cascade d'entrée */
    .hero-section .hero-title {
      animation: faso-hero-rise 900ms cubic-bezier(0.2, 0.0, 0.2, 1) both;
      animation-delay: 80ms;
    }
    .hero-section .hero-subtitle {
      animation: faso-hero-rise 900ms cubic-bezier(0.2, 0.0, 0.2, 1) both;
      animation-delay: 240ms;
    }
    .hero-section .hero-ctas {
      animation: faso-hero-rise 900ms cubic-bezier(0.2, 0.0, 0.2, 1) both;
      animation-delay: 400ms;
    }
    .hero-section .hero-illustration {
      animation: faso-hero-pop 1000ms cubic-bezier(0.34, 1.56, 0.64, 1) both;
      animation-delay: 560ms;
    }
    .hero-section .hero-ctas a {
      transition: transform 240ms cubic-bezier(0, 0, 0.2, 1),
                  box-shadow 240ms cubic-bezier(0, 0, 0.2, 1);
    }
    .hero-section .hero-ctas a:hover {
      transform: translateY(-3px);
      box-shadow: 0 12px 24px rgba(0, 0, 0, 0.18);
    }

    /* Formes de fond qui flottent en continu */
    .hero-bg-shapes .shape {
      animation: faso-shape-drift 16s ease-in-out infinite;
      will-change: transform;
    }
    .hero-bg-shapes .shape-1 { animation-duration: 18s; animation-delay: 0s; }
    .hero-bg-shapes .shape-2 { animation-duration: 22s; animation-delay: -5s; }
    .hero-bg-shapes .shape-3 { animation-duration: 20s; animation-delay: -10s; }

    /* Hero SVG : les poulets et grains flottent */
    .hero-svg g[transform^="translate(180"] {
      animation: faso-float-subtle 3.6s ease-in-out infinite;
      transform-origin: center;
      transform-box: fill-box;
    }
    .hero-svg g[transform^="translate(260"] {
      animation: faso-float-subtle 4.2s ease-in-out infinite;
      animation-delay: -1.2s;
      transform-origin: center;
      transform-box: fill-box;
    }
    .hero-svg g[transform^="translate(320"] {
      animation: faso-float-bigger 3.1s ease-in-out infinite;
      animation-delay: -0.5s;
      transform-origin: center;
      transform-box: fill-box;
    }
    .hero-svg circle[fill="#FCD116"] {
      animation: faso-shine 4s ease-in-out infinite;
    }

    /* Stats bar : le comptage est géré par JS, on boost l'entrée visuelle */
    .hero-stats {
      animation: faso-fade-in-basic 1200ms ease-out both;
      animation-delay: 800ms;
    }

    /* Navbar : entrée top-down (translateX(-50%) conservé pour le centrage) */
    @keyframes faso-nav-drop {
      from { opacity: 0; transform: translate(-50%, -120%); }
      to   { opacity: 1; transform: translate(-50%, 0); }
    }
    .landing-nav {
      animation: faso-nav-drop 520ms cubic-bezier(0.2, 0.0, 0.2, 1) both;
    }

    /* Boost hover sur les fade-in-cards une fois visibles */
    .fade-in-card.visible {
      will-change: transform, box-shadow;
    }
    .fade-in-card.visible:hover {
      transform: translateY(-6px) scale(1.02);
      box-shadow: 0 16px 40px rgba(0, 0, 0, 0.12);
    }

    /* ===================================================================
       SECTION "L'AVICULTURE RÉINVENTÉE" — 3 piliers glass
       (miroir de la section "Production evolved" — claude.ai design)
       =================================================================== */
    .reinvented-section {
      position: relative;
      padding: clamp(64px, 8vw, 120px) 24px;
      background: linear-gradient(180deg, #0D1F12 0%, #1A3A22 55%, #2A4D30 100%);
      overflow: hidden;
      color: white;
      isolation: isolate;
    }

    .reinvented-bg {
      position: absolute;
      inset: 0;
      pointer-events: none;
      overflow: hidden;
      z-index: 0;
    }
    .reinvented-bg .blob {
      position: absolute;
      border-radius: 50%;
      filter: blur(90px);
      opacity: 0.55;
      will-change: transform;
      animation: reinvented-blob-float 22s ease-in-out infinite alternate;
    }
    .reinvented-bg .blob-1 {
      width: 520px; height: 520px;
      background: rgba(252, 209, 22, 0.28);
      top: -180px; left: -120px;
      animation-delay: 0s;
    }
    .reinvented-bg .blob-2 {
      width: 560px; height: 560px;
      background: rgba(0, 158, 73, 0.32);
      bottom: -200px; right: -140px;
      animation-delay: -8s;
    }
    .reinvented-bg .blob-3 {
      width: 420px; height: 420px;
      background: rgba(239, 43, 45, 0.22);
      top: 35%; left: 50%; transform: translateX(-50%);
      animation-delay: -14s;
    }
    @keyframes reinvented-blob-float {
      from { transform: translate3d(0, 0, 0) scale(1); }
      to   { transform: translate3d(4%, -3%, 0) scale(1.06); }
    }

    .reinvented-container {
      position: relative;
      z-index: 1;
      max-width: 1180px;
      margin: 0 auto;
    }
    .reinvented-head {
      text-align: center;
      max-width: 720px;
      margin: 0 auto 56px;
    }
    .reinvented-eyebrow {
      display: inline-block;
      font-family: 'Roboto', system-ui, sans-serif;
      font-size: 0.75rem;
      font-weight: 600;
      text-transform: uppercase;
      letter-spacing: 0.16em;
      color: var(--gold, #FCD116);
      margin-bottom: 14px;
    }
    .reinvented-title {
      font-family: 'Fraunces', 'Playfair Display', Georgia, serif;
      font-style: italic;
      font-optical-sizing: auto;
      font-weight: 500;
      font-size: clamp(2rem, 4.5vw, 3.2rem);
      line-height: 1.1;
      letter-spacing: -0.015em;
      margin: 0 0 18px;
      text-shadow: 0 2px 24px rgba(0, 0, 0, 0.35);
    }
    .reinvented-lead {
      font-family: 'Roboto', system-ui, sans-serif;
      font-size: clamp(1rem, 1.3vw, 1.15rem);
      line-height: 1.6;
      opacity: 0.82;
      margin: 0;
    }

    .reinvented-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
      gap: 22px;
    }
    .reinvented-card {
      position: relative;
      padding: 32px 28px 28px;
      background: rgba(255, 255, 255, 0.06);
      backdrop-filter: blur(18px) saturate(140%);
      -webkit-backdrop-filter: blur(18px) saturate(140%);
      border: 1px solid rgba(255, 255, 255, 0.14);
      border-radius: 22px;
      box-shadow:
        0 12px 40px rgba(0, 0, 0, 0.25),
        inset 0 1px 0 rgba(255, 255, 255, 0.10);
      display: flex;
      flex-direction: column;
      gap: 12px;
      transition:
        transform 320ms cubic-bezier(0.2, 0.0, 0.2, 1),
        border-color 320ms ease,
        box-shadow 320ms ease;
    }
    .reinvented-card:hover {
      transform: translateY(-6px);
      border-color: rgba(255, 255, 255, 0.26);
      box-shadow:
        0 22px 60px rgba(0, 0, 0, 0.35),
        inset 0 1px 0 rgba(255, 255, 255, 0.18);
    }
    .reinvented-icon {
      width: 52px;
      height: 52px;
      display: grid;
      place-items: center;
      border-radius: 14px;
      background: linear-gradient(135deg, rgba(252, 209, 22, 0.22), rgba(252, 209, 22, 0.06));
      border: 1px solid rgba(252, 209, 22, 0.28);
      color: var(--gold, #FCD116);
      margin-bottom: 4px;
    }
    .reinvented-icon mat-icon {
      font-size: 26px; width: 26px; height: 26px;
    }
    .reinvented-card-title {
      font-family: 'Fraunces', 'Playfair Display', Georgia, serif;
      font-style: italic;
      font-weight: 500;
      font-size: 1.55rem;
      line-height: 1.2;
      letter-spacing: -0.01em;
      margin: 0;
    }
    .reinvented-card-desc {
      font-family: 'Roboto', system-ui, sans-serif;
      font-size: 0.95rem;
      line-height: 1.55;
      opacity: 0.82;
      margin: 0;
    }
    .reinvented-pills {
      display: flex;
      flex-wrap: wrap;
      gap: 6px;
      margin-top: auto;
      padding-top: 10px;
    }
    .reinvented-pill {
      font-family: 'Roboto', system-ui, sans-serif;
      font-size: 0.72rem;
      font-weight: 500;
      padding: 4px 10px;
      border-radius: 999px;
      background: rgba(255, 255, 255, 0.06);
      border: 1px solid rgba(255, 255, 255, 0.14);
      color: rgba(255, 255, 255, 0.88);
      white-space: nowrap;
    }

    /* Respect des préférences utilisateur */
    @media (prefers-reduced-motion: reduce) {
      .hero-section .hero-title,
      .hero-section .hero-subtitle,
      .hero-section .hero-ctas,
      .hero-section .hero-illustration,
      .hero-bg-shapes .shape,
      .hero-svg g,
      .hero-svg circle,
      .hero-stats,
      .hero-atmosphere,
      .hero-video,
      .landing-nav,
      .reinvented-bg .blob {
        animation: none !important;
      }
      .fade-in-card.visible:hover,
      .reinvented-card:hover,
      .stat-item:hover {
        transform: none !important;
      }
    }
  `],
})
export class LandingComponent implements OnInit, AfterViewInit, OnDestroy {
  private readonly el = inject(ElementRef);
  private readonly platformId = inject(PLATFORM_ID);
  private readonly translate = inject(TranslateService);
  private readonly viewportScroller = inject(ViewportScroller);
  private readonly zone = inject(NgZone);

  readonly navScrolled = signal(false);
  readonly mobileMenuOpen = signal(false);
  readonly statsVisible = signal(false);
  readonly cardsVisible = signal<boolean[]>([false, false, false]);
  readonly featuresVisible = signal<boolean[]>(new Array(9).fill(false));
  readonly securityVisible = signal<boolean[]>(new Array(6).fill(false));
  readonly currentLang = signal(this.translate.currentLang || this.translate.defaultLang || 'fr');

  // Hero video : opt-in, désactivé sous reduced-motion / save-data
  readonly heroVideoEnabled = signal(this.computeHeroVideoEnabled());
  readonly heroVideoReady = signal(false);

  private scrollListener: (() => void) | null = null;
  private observers: IntersectionObserver[] = [];

  // Trois piliers (section "L'aviculture réinventée")
  readonly reinventedCards = [
    {
      id: 'halal',
      icon: 'verified',
      titleKey: 'landing.reinvented.cards.halal.title',
      descKey: 'landing.reinvented.cards.halal.desc',
      pills: [
        'landing.reinvented.cards.halal.p1',
        'landing.reinvented.cards.halal.p2',
        'landing.reinvented.cards.halal.p3',
      ],
    },
    {
      id: 'payment',
      icon: 'smartphone',
      titleKey: 'landing.reinvented.cards.payment.title',
      descKey: 'landing.reinvented.cards.payment.desc',
      pills: [
        'landing.reinvented.cards.payment.p1',
        'landing.reinvented.cards.payment.p2',
        'landing.reinvented.cards.payment.p3',
      ],
    },
    {
      id: 'delivery',
      icon: 'local_shipping',
      titleKey: 'landing.reinvented.cards.delivery.title',
      descKey: 'landing.reinvented.cards.delivery.desc',
      pills: [
        'landing.reinvented.cards.delivery.p1',
        'landing.reinvented.cards.delivery.p2',
        'landing.reinvented.cards.delivery.p3',
      ],
    },
  ];

  private computeHeroVideoEnabled(): boolean {
    if (!isPlatformBrowser(this.platformId)) return false;
    if (typeof window === 'undefined') return false;
    const reduced = window.matchMedia?.('(prefers-reduced-motion: reduce)').matches;
    const conn = (navigator as unknown as { connection?: { saveData?: boolean; effectiveType?: string } }).connection;
    const saveData = !!conn?.saveData;
    const slow = conn?.effectiveType === '2g' || conn?.effectiveType === 'slow-2g';
    return !(reduced || saveData || slow);
  }

  onHeroVideoReady(): void {
    this.heroVideoReady.set(true);
  }
  onHeroVideoError(): void {
    this.heroVideoEnabled.set(false);
    this.heroVideoReady.set(false);
  }

  // Steps data
  readonly steps = [
    { num: 1, icon: 'person_add', titleKey: 'landing.presentation.step1_title', descKey: 'landing.presentation.step1_desc' },
    { num: 2, icon: 'search', titleKey: 'landing.presentation.step2_title', descKey: 'landing.presentation.step2_desc' },
    { num: 3, icon: 'handshake', titleKey: 'landing.presentation.step3_title', descKey: 'landing.presentation.step3_desc' },
  ];

  // Features data
  readonly features = [
    { icon: 'store', titleKey: 'landing.features.f1_title', descKey: 'landing.features.f1_desc' },
    { icon: 'calendar_month', titleKey: 'landing.features.f2_title', descKey: 'landing.features.f2_desc' },
    { icon: 'autorenew', titleKey: 'landing.features.f3_title', descKey: 'landing.features.f3_desc' },
    { icon: 'trending_up', titleKey: 'landing.features.f4_title', descKey: 'landing.features.f4_desc' },
    { icon: 'vaccines', titleKey: 'landing.features.f5_title', descKey: 'landing.features.f5_desc' },
    { icon: 'verified', titleKey: 'landing.features.f6_title', descKey: 'landing.features.f6_desc' },
    { icon: 'chat', titleKey: 'landing.features.f7_title', descKey: 'landing.features.f7_desc' },
    { icon: 'local_shipping', titleKey: 'landing.features.f8_title', descKey: 'landing.features.f8_desc' },
    { icon: 'groups', titleKey: 'landing.features.f9_title', descKey: 'landing.features.f9_desc' },
  ];

  // Security data
  readonly securityItems = [
    { icon: 'shield', titleKey: 'landing.security.s1_title', descKey: 'landing.security.s1_desc' },
    { icon: 'fingerprint', titleKey: 'landing.security.s2_title', descKey: 'landing.security.s2_desc' },
    { icon: 'account_balance_wallet', titleKey: 'landing.security.s3_title', descKey: 'landing.security.s3_desc' },
    { icon: 'fact_check', titleKey: 'landing.security.s4_title', descKey: 'landing.security.s4_desc' },
    { icon: 'star_rate', titleKey: 'landing.security.s5_title', descKey: 'landing.security.s5_desc' },
    { icon: 'dns', titleKey: 'landing.security.s6_title', descKey: 'landing.security.s6_desc' },
  ];

  // Platforms data
  readonly platforms = [
    { icon: 'badge', labelKey: 'landing.about.p_etat_civil' },
    { icon: 'local_hospital', labelKey: 'landing.about.p_sante' },
    { icon: 'agriculture', labelKey: 'landing.about.p_agriculture' },
    { icon: 'storefront', labelKey: 'landing.about.p_commerce' },
    { icon: 'school', labelKey: 'landing.about.p_education' },
    { icon: 'directions_bus', labelKey: 'landing.about.p_transport' },
    { icon: 'egg_alt', labelKey: 'landing.about.p_volailles' },
  ];

  ngOnInit(): void {
    if (isPlatformBrowser(this.platformId)) {
      this.zone.runOutsideAngular(() => {
        this.scrollListener = () => {
          const scrolled = window.scrollY > 60;
          if (this.navScrolled() !== scrolled) {
            this.zone.run(() => this.navScrolled.set(scrolled));
          }
        };
        window.addEventListener('scroll', this.scrollListener, { passive: true });
      });
    }
  }

  ngAfterViewInit(): void {
    if (!isPlatformBrowser(this.platformId)) return;

    // Observe stats bar
    this.observeElement('.hero-stats', () => {
      this.statsVisible.set(true);
    });

    // Observe step cards
    this.observeElements('.steps-grid .step-card', (index: number) => {
      const current = [...this.cardsVisible()];
      current[index] = true;
      this.cardsVisible.set(current);
    });

    // Observe feature cards
    this.observeElements('.features-grid .feature-card', (index: number) => {
      const current = [...this.featuresVisible()];
      current[index] = true;
      this.featuresVisible.set(current);
    });

    // Observe security cards
    this.observeElements('.security-grid .security-card', (index: number) => {
      const current = [...this.securityVisible()];
      current[index] = true;
      this.securityVisible.set(current);
    });
  }

  ngOnDestroy(): void {
    if (this.scrollListener && isPlatformBrowser(this.platformId)) {
      window.removeEventListener('scroll', this.scrollListener);
    }
    this.observers.forEach(obs => obs.disconnect());
  }

  scrollTo(sectionId: string): void {
    const element = this.el.nativeElement.querySelector(`#${sectionId}`);
    if (element) {
      const offset = 72; // nav height
      const top = element.getBoundingClientRect().top + window.scrollY - offset;
      window.scrollTo({ top, behavior: 'smooth' });
    }
  }

  toggleMobile(): void {
    this.mobileMenuOpen.set(!this.mobileMenuOpen());
  }

  closeMobile(): void {
    this.mobileMenuOpen.set(false);
  }

  switchLang(lang: string): void {
    this.translate.use(lang);
    this.currentLang.set(lang);
    try {
      localStorage.setItem('faso_lang', lang);
    } catch {
      // Storage unavailable
    }
  }

  private observeElement(selector: string, callback: () => void): void {
    const el = this.el.nativeElement.querySelector(selector);
    if (!el) return;

    const observer = new IntersectionObserver(
      (entries) => {
        entries.forEach((entry) => {
          if (entry.isIntersecting) {
            this.zone.run(() => callback());
            observer.unobserve(entry.target);
          }
        });
      },
      { threshold: 0.2 }
    );

    observer.observe(el);
    this.observers.push(observer);
  }

  private observeElements(selector: string, callback: (index: number) => void): void {
    const elements = this.el.nativeElement.querySelectorAll(selector);
    if (!elements.length) return;

    const observer = new IntersectionObserver(
      (entries) => {
        entries.forEach((entry) => {
          if (entry.isIntersecting) {
            const index = Array.from(elements).indexOf(entry.target);
            if (index >= 0) {
              this.zone.run(() => callback(index));
            }
            observer.unobserve(entry.target);
          }
        });
      },
      { threshold: 0.15 }
    );

    elements.forEach((el: Element) => observer.observe(el));
    this.observers.push(observer);
  }
}
