import { Component, ChangeDetectionStrategy } from '@angular/core';
import { TranslateModule } from '@ngx-translate/core';
import { EmptyStateComponent } from '@shared/components/empty-state/empty-state.component';

@Component({
  selector: 'app-map-home',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [TranslateModule, EmptyStateComponent],
  template: `
    <div class="page-container">
      <h1>{{ 'menu.map' | translate }}</h1>
      <app-empty-state icon="map" title="common.no_data"></app-empty-state>
    </div>
  `,
  styles: [`.page-container { padding: 24px; max-width: 1200px; margin: 0 auto; }`],
})
export class MapHomeComponent {}
