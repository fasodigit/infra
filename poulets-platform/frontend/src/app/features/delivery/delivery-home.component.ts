import { Component, ChangeDetectionStrategy } from '@angular/core';
import { TranslateModule } from '@ngx-translate/core';
import { EmptyStateComponent } from '@shared/components/empty-state/empty-state.component';

@Component({
  selector: 'app-delivery-home',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [TranslateModule, EmptyStateComponent],
  template: `
    <div class="page-container">
      <h1>{{ 'delivery.title' | translate }}</h1>
      <app-empty-state icon="local_shipping" title="delivery.no_deliveries"></app-empty-state>
    </div>
  `,
  styles: [`.page-container { padding: 24px; max-width: 1200px; margin: 0 auto; }`],
})
export class DeliveryHomeComponent {}
