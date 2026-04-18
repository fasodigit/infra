// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, OnInit, inject, signal } from '@angular/core';
import { CommonModule, DatePipe } from '@angular/common';
import { RouterLink } from '@angular/router';
import { MatIconModule } from '@angular/material/icon';
import { MatButtonModule } from '@angular/material/button';
import { LoadingComponent } from '@shared/components/loading/loading.component';
import { ImpressionService, RenderTemplate } from '../services/impression.service';

@Component({
  selector: 'app-templates-browser',
  standalone: true,
  imports: [CommonModule, DatePipe, RouterLink, MatIconModule, MatButtonModule, LoadingComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="page">
      <header>
        <a mat-button routerLink=".." class="back">
          <mat-icon>arrow_back</mat-icon> Retour
        </a>
        <div>
          <h1>Templates Handlebars</h1>
          <p>Templates disponibles dans <code>ec-certificate-renderer</code> — édition via le dépôt backend</p>
        </div>
      </header>

      @if (loading()) {
        <app-loading message="Chargement des templates…" />
      } @else {
        <div class="grid">
          @for (t of templates(); track t.name) {
            <article class="card">
              <header>
                <mat-icon>description</mat-icon>
                <div>
                  <strong>{{ t.label }}</strong>
                  <code>{{ t.name }}.hbs</code>
                </div>
              </header>
              <p>{{ t.description }}</p>
              <div class="vars">
                <small>Variables requises</small>
                <div>
                  @for (v of t.variables; track v) {
                    <code>{{ v }}</code>
                  }
                </div>
              </div>
              <footer>
                <small>Mis à jour {{ t.updatedAt | date:'mediumDate' }}</small>
                <a mat-stroked-button [routerLink]="['../test', t.name]">
                  <mat-icon>play_arrow</mat-icon> Tester
                </a>
              </footer>
            </article>
          }
        </div>
      }
    </section>
  `,
  styles: [`
    :host { display: block; }
    .back { margin-left: calc(var(--faso-space-4) * -1); color: var(--faso-text-muted); margin-bottom: var(--faso-space-2); display: inline-flex; }
    header > div { }
    header h1 { margin: 0; font-size: var(--faso-text-3xl); font-weight: var(--faso-weight-bold); }
    header p { margin: 4px 0 0; color: var(--faso-text-muted); }
    header code {
      font-family: var(--faso-font-mono);
      background: var(--faso-surface-alt);
      padding: 2px 6px;
      border-radius: var(--faso-radius-sm);
      font-size: var(--faso-text-sm);
    }

    .grid {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
      gap: var(--faso-space-4);
      margin-top: var(--faso-space-5);
    }
    .card {
      padding: var(--faso-space-4);
      background: var(--faso-surface);
      border: 1px solid var(--faso-border);
      border-radius: var(--faso-radius-xl);
      display: flex;
      flex-direction: column;
      gap: var(--faso-space-3);
    }
    .card header {
      display: flex;
      gap: var(--faso-space-3);
      align-items: center;
      margin: 0;
    }
    .card > header mat-icon {
      width: 44px; height: 44px;
      background: var(--faso-primary-50);
      color: var(--faso-primary-700);
      border-radius: 12px;
      display: inline-flex;
      align-items: center;
      justify-content: center;
      font-size: 24px;
    }
    .card strong { display: block; font-size: var(--faso-text-lg); }
    .card > header code {
      display: block;
      font-family: var(--faso-font-mono);
      color: var(--faso-text-muted);
      font-size: var(--faso-text-xs);
      margin-top: 2px;
    }
    .card p { margin: 0; color: var(--faso-text-muted); }
    .vars small {
      display: block;
      color: var(--faso-text-muted);
      font-size: var(--faso-text-xs);
      text-transform: uppercase;
      letter-spacing: 0.04em;
      margin-bottom: 4px;
    }
    .vars > div { display: flex; flex-wrap: wrap; gap: 4px; }
    .vars code {
      padding: 2px 8px;
      background: var(--faso-accent-100);
      color: var(--faso-accent-800);
      border-radius: var(--faso-radius-sm);
      font-family: var(--faso-font-mono);
      font-size: var(--faso-text-xs);
    }
    .card footer {
      display: flex;
      justify-content: space-between;
      align-items: center;
      margin-top: auto;
      padding-top: var(--faso-space-2);
      border-top: 1px solid var(--faso-border);
    }
    .card footer small { color: var(--faso-text-subtle); font-size: var(--faso-text-xs); }
  `],
})
export class TemplatesBrowserComponent implements OnInit {
  private readonly svc = inject(ImpressionService);

  readonly templates = signal<RenderTemplate[]>([]);
  readonly loading = signal(true);

  ngOnInit(): void {
    this.svc.listTemplates().subscribe({
      next: (arr) => { this.templates.set(arr); this.loading.set(false); },
      error: () => this.loading.set(false),
    });
  }
}
