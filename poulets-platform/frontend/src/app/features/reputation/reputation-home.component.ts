import { Component, ChangeDetectionStrategy } from '@angular/core';
import { TranslateModule } from '@ngx-translate/core';
import { EmptyStateComponent } from '@shared/components/empty-state/empty-state.component';

@Component({
  selector: 'app-reputation-home',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [TranslateModule, EmptyStateComponent],
  template: `
    <div class="page-container">
      <h1>{{ 'reputation.title' | translate }}</h1>
      <app-empty-state icon="star" title="reputation.no_reviews"></app-empty-state>
    </div>
  `,
  styles: [`.page-container { padding: 24px; max-width: 1200px; margin: 0 auto; }`],
})
export class ReputationHomeComponent {}
