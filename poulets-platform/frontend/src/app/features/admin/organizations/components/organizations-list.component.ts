// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import { ChangeDetectionStrategy, Component, OnInit, inject, signal } from '@angular/core';
import { CommonModule, DecimalPipe } from '@angular/common';
import { RouterLink } from '@angular/router';
import { MatIconModule } from '@angular/material/icon';
import { MatButtonModule } from '@angular/material/button';
import { LoadingComponent } from '@shared/components/loading/loading.component';
import { RatingStarsComponent } from '@shared/components/rating-stars/rating-stars.component';
import { OrganizationsService, Organization } from '../services/organizations.service';

@Component({
  selector: 'app-organizations-list',
  standalone: true,
  imports: [CommonModule, DecimalPipe, RouterLink, MatIconModule, MatButtonModule, LoadingComponent, RatingStarsComponent],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <section class="page">
      <header>
        <div>
          <h1>Groupements &amp; coopératives</h1>
          <p>{{ orgs().length }} organisations enregistrées</p>
        </div>
        <a mat-raised-button color="primary" routerLink="create">
          <mat-icon>add</mat-icon> Créer une organisation
        </a>
      </header>

      @if (loading()) {
        <app-loading message="Chargement…" />
      } @else if (orgs().length === 0) {
        <p class="empty">Aucune organisation.</p>
      } @else {
        <div class="grid">
          @for (o of orgs(); track o.id) {
            <article class="card">
              <header class="card-head">
                <mat-icon>{{ iconFor(o.type) }}</mat-icon>
                <div>
                  <a [routerLink]="[o.id]"><strong>{{ o.name }}</strong></a>
                  <small>{{ typeLabel(o.type) }} · {{ o.region }}</small>
                </div>
              </header>
              <dl>
                <div><dt>Membres</dt><dd>{{ o.activeMembers }} / {{ o.memberCount }} actifs</dd></div>
                <div><dt>Ventes totales</dt><dd>{{ o.totalSales | number:'1.0-0' }}</dd></div>
                <div><dt>Note</dt><dd><app-rating-stars [value]="o.avgRating" [showValue]="true" /></dd></div>
              </dl>
              <div class="certs">
                @for (c of o.certifications; track c) {
                  <span class="cert-pill">{{ c }}</span>
                }
              </div>
              <footer>
                <span>{{ o.contactName }} · {{ o.contactPhone }}</span>
                <a mat-button [routerLink]="[o.id]">Détails →</a>
              </footer>
            </article>
          }
        </div>
      }
    </section>
  `,
  styles: [`
    :host { display: block; }
    header {
      display: flex;
      justify-content: space-between;
      align-items: flex-end;
      gap: var(--faso-space-3);
      margin-bottom: var(--faso-space-5);
      flex-wrap: wrap;
    }
    header h1 { margin: 0; font-size: var(--faso-text-3xl); font-weight: var(--faso-weight-bold); }
    header p { margin: 4px 0 0; color: var(--faso-text-muted); }

    .empty { padding: var(--faso-space-10); text-align: center; color: var(--faso-text-muted); }

    .grid {
      display: grid;
      grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
      gap: var(--faso-space-4);
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
    .card-head {
      display: flex;
      gap: var(--faso-space-3);
      align-items: center;
      margin: 0;
    }
    .card-head > mat-icon {
      width: 44px; height: 44px;
      border-radius: 12px;
      background: var(--faso-primary-50);
      color: var(--faso-primary-700);
      display: inline-flex;
      align-items: center;
      justify-content: center;
    }
    .card-head a { color: var(--faso-text); text-decoration: none; }
    .card-head a:hover { color: var(--faso-primary-700); }
    .card-head small { display: block; color: var(--faso-text-muted); font-size: var(--faso-text-sm); }

    dl { margin: 0; display: flex; flex-direction: column; gap: 4px; }
    dl div { display: flex; justify-content: space-between; font-size: var(--faso-text-sm); padding: 2px 0; }
    dl dt { color: var(--faso-text-muted); }
    dl dd { margin: 0; font-weight: var(--faso-weight-medium); }

    .certs { display: flex; gap: 4px; flex-wrap: wrap; }
    .cert-pill {
      padding: 2px 10px;
      background: var(--faso-primary-50);
      color: var(--faso-primary-700);
      border-radius: var(--faso-radius-pill);
      font-size: var(--faso-text-xs);
      font-weight: var(--faso-weight-semibold);
      text-transform: uppercase;
      letter-spacing: 0.04em;
    }

    footer {
      margin-top: auto;
      padding-top: var(--faso-space-3);
      border-top: 1px solid var(--faso-border);
      display: flex;
      justify-content: space-between;
      align-items: center;
      gap: var(--faso-space-2);
      color: var(--faso-text-muted);
      font-size: var(--faso-text-sm);
    }
  `],
})
export class OrganizationsListComponent implements OnInit {
  private readonly svc = inject(OrganizationsService);
  readonly orgs = signal<Organization[]>([]);
  readonly loading = signal(true);

  ngOnInit(): void {
    this.svc.list().subscribe((arr) => {
      this.orgs.set(arr);
      this.loading.set(false);
    });
  }

  iconFor(t: Organization['type']): string {
    switch (t) {
      case 'COOPERATIVE': return 'groups';
      case 'GROUPEMENT':  return 'group_work';
      case 'ASSOCIATION': return 'workspaces';
    }
  }
  typeLabel(t: Organization['type']): string {
    switch (t) {
      case 'COOPERATIVE': return 'Coopérative';
      case 'GROUPEMENT':  return 'Groupement';
      case 'ASSOCIATION': return 'Association';
    }
  }
}
