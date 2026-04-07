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
    <section id="accueil" class="hero-section">
      <div class="hero-bg-shapes">
        <div class="shape shape-1"></div>
        <div class="shape shape-2"></div>
        <div class="shape shape-3"></div>
      </div>
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
          <svg viewBox="0 0 400 350" xmlns="http://www.w3.org/2000/svg" class="hero-svg">
            <!-- Farm scene -->
            <defs>
              <linearGradient id="skyGrad" x1="0" y1="0" x2="0" y2="1">
                <stop offset="0%" stop-color="#87CEEB" stop-opacity="0.3"/>
                <stop offset="100%" stop-color="#FCD116" stop-opacity="0.15"/>
              </linearGradient>
              <linearGradient id="grassGrad" x1="0" y1="0" x2="0" y2="1">
                <stop offset="0%" stop-color="#4CAF50"/>
                <stop offset="100%" stop-color="#2E7D32"/>
              </linearGradient>
            </defs>
            <!-- Sky -->
            <rect x="0" y="0" width="400" height="350" fill="url(#skyGrad)" rx="16"/>
            <!-- Sun -->
            <circle cx="320" cy="60" r="35" fill="#FCD116" opacity="0.9"/>
            <circle cx="320" cy="60" r="45" fill="#FCD116" opacity="0.2"/>
            <!-- Clouds -->
            <ellipse cx="80" cy="50" rx="40" ry="15" fill="white" opacity="0.7"/>
            <ellipse cx="100" cy="45" rx="30" ry="12" fill="white" opacity="0.6"/>
            <ellipse cx="220" cy="35" rx="35" ry="12" fill="white" opacity="0.5"/>
            <!-- Ground -->
            <ellipse cx="200" cy="310" rx="200" ry="60" fill="url(#grassGrad)" opacity="0.8"/>
            <!-- Barn -->
            <rect x="40" y="160" width="90" height="100" fill="#D32F2F" rx="4"/>
            <polygon points="85,120 30,165 140,165" fill="#B71C1C"/>
            <rect x="70" y="200" width="30" height="60" fill="#5D4037"/>
            <rect x="50" y="180" width="20" height="20" fill="#FFF9C4" opacity="0.8" rx="2"/>
            <rect x="100" y="180" width="20" height="20" fill="#FFF9C4" opacity="0.8" rx="2"/>
            <!-- Fence -->
            <line x1="150" y1="260" x2="360" y2="260" stroke="#8D6E63" stroke-width="3"/>
            <line x1="150" y1="245" x2="360" y2="245" stroke="#8D6E63" stroke-width="2"/>
            @for (i of [160,190,220,250,280,310,340]; track i) {
              <line [attr.x1]="i" y1="235" [attr.x2]="i" y2="270" stroke="#8D6E63" stroke-width="3"/>
            }
            <!-- Chicken 1 -->
            <g transform="translate(180, 220)">
              <ellipse cx="0" cy="0" rx="18" ry="14" fill="#F5F5F5"/>
              <circle cx="-14" cy="-8" r="8" fill="#F5F5F5"/>
              <circle cx="-16" cy="-10" r="2" fill="#333"/>
              <polygon points="-22,-8 -28,-6 -22,-5" fill="#FF8F00"/>
              <polygon points="-12,-16 -10,-22 -8,-16 -6,-20 -4,-14" fill="#EF2B2D"/>
              <line x1="6" y1="12" x2="4" y2="24" stroke="#FF8F00" stroke-width="2"/>
              <line x1="-4" y1="12" x2="-6" y2="24" stroke="#FF8F00" stroke-width="2"/>
              <line x1="12" y1="-2" x2="22" y2="4" stroke="#F5F5F5" stroke-width="4"/>
              <polygon points="20,2 26,0 22,6" fill="#E0E0E0"/>
            </g>
            <!-- Chicken 2 -->
            <g transform="translate(260, 230)">
              <ellipse cx="0" cy="0" rx="16" ry="12" fill="#8D6E63"/>
              <circle cx="12" cy="-6" r="7" fill="#8D6E63"/>
              <circle cx="14" cy="-8" r="1.8" fill="#333"/>
              <polygon points="19,-6 25,-4 19,-3" fill="#FF8F00"/>
              <polygon points="10,-13 12,-18 14,-12 16,-16 18,-11" fill="#EF2B2D"/>
              <line x1="-4" y1="10" x2="-6" y2="22" stroke="#FF8F00" stroke-width="2"/>
              <line x1="4" y1="10" x2="2" y2="22" stroke="#FF8F00" stroke-width="2"/>
            </g>
            <!-- Chicken 3 (small chick) -->
            <g transform="translate(320, 238)">
              <ellipse cx="0" cy="0" rx="9" ry="8" fill="#FDD835"/>
              <circle cx="-7" cy="-4" r="5" fill="#FDD835"/>
              <circle cx="-8" cy="-5" r="1.3" fill="#333"/>
              <polygon points="-12,-4 -16,-3 -12,-2" fill="#FF8F00"/>
            </g>
            <!-- Grain -->
            @for (g of grainDots; track g.x) {
              <circle [attr.cx]="g.x" [attr.cy]="g.y" r="1.5" fill="#8D6E63" opacity="0.5"/>
            }
          </svg>
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
      top: 0;
      left: 0;
      right: 0;
      z-index: 1000;
      padding: 0 24px;
      height: 72px;
      transition: background-color var(--transition), box-shadow var(--transition), backdrop-filter var(--transition);
      background: transparent;
    }

    .landing-nav.nav-scrolled {
      background: rgba(255, 255, 255, 0.97);
      backdrop-filter: blur(12px);
      box-shadow: 0 2px 20px rgba(0, 0, 0, 0.08);
    }

    .nav-inner {
      max-width: 1280px;
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
      .nav-links {
        position: fixed;
        top: 72px;
        left: 0;
        right: 0;
        bottom: 0;
        background: rgba(255, 255, 255, 0.98);
        backdrop-filter: blur(12px);
        flex-direction: column;
        align-items: stretch;
        justify-content: flex-start;
        padding: 24px;
        gap: 4px;
        transform: translateX(100%);
        transition: transform 0.35s cubic-bezier(0.4, 0, 0.2, 1);
        overflow-y: auto;
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
      padding-top: 72px;
    }

    .hero-bg-shapes {
      position: absolute;
      inset: 0;
      pointer-events: none;
      overflow: hidden;
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
      font-size: 2.75rem;
      font-weight: 800;
      line-height: 1.15;
      margin: 0 0 20px;
      letter-spacing: -0.5px;
    }

    .hero-subtitle {
      font-size: 1.15rem;
      line-height: 1.7;
      margin: 0 0 36px;
      opacity: 0.92;
      max-width: 520px;
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

    /* Stats bar */
    .hero-stats {
      display: flex;
      justify-content: center;
      gap: 0;
      background: rgba(255, 255, 255, 0.12);
      backdrop-filter: blur(12px);
      border-top: 1px solid rgba(255, 255, 255, 0.15);
      position: relative;
      z-index: 1;
    }

    .stat-item {
      flex: 1;
      max-width: 280px;
      text-align: center;
      padding: 28px 16px;
      color: white;
      border-right: 1px solid rgba(255, 255, 255, 0.1);
      opacity: 0;
      transform: translateY(20px);
      transition: opacity 0.6s ease, transform 0.6s ease;
    }

    .stat-item:last-child {
      border-right: none;
    }

    .stat-item.animate-stat {
      opacity: 1;
      transform: translateY(0);
    }

    .stat-item:nth-child(2).animate-stat { transition-delay: 0.1s; }
    .stat-item:nth-child(3).animate-stat { transition-delay: 0.2s; }
    .stat-item:nth-child(4).animate-stat { transition-delay: 0.3s; }

    .stat-number {
      display: block;
      font-size: 2rem;
      font-weight: 800;
      letter-spacing: -0.5px;
    }

    .stat-label {
      display: block;
      font-size: 0.85rem;
      opacity: 0.85;
      margin-top: 4px;
      text-transform: uppercase;
      letter-spacing: 0.5px;
    }

    @media (max-width: 900px) {
      .hero-content {
        grid-template-columns: 1fr;
        padding: 40px 24px;
        gap: 32px;
        text-align: center;
      }

      .hero-title {
        font-size: 2rem;
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
      }

      .hero-svg {
        max-width: 280px;
      }

      .hero-stats {
        flex-wrap: wrap;
      }

      .stat-item {
        flex: 1 1 50%;
        padding: 20px 12px;
        border-right: none;
      }

      .stat-item:nth-child(1),
      .stat-item:nth-child(2) {
        border-bottom: 1px solid rgba(255, 255, 255, 0.1);
      }

      .stat-number {
        font-size: 1.5rem;
      }
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
    @media (max-width: 600px) {
      .section {
        padding: 56px 16px;
      }

      .section-title {
        font-size: 1.75rem;
      }

      .section-subtitle {
        font-size: 1rem;
        margin-bottom: 40px;
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

  private scrollListener: (() => void) | null = null;
  private observers: IntersectionObserver[] = [];

  // SVG grain dots
  readonly grainDots = [
    { x: 200, y: 250 }, { x: 210, y: 255 }, { x: 195, y: 258 },
    { x: 270, y: 252 }, { x: 265, y: 248 }, { x: 280, y: 255 },
    { x: 310, y: 250 }, { x: 325, y: 252 },
  ];

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
