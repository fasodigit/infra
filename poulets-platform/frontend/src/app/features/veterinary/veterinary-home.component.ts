import { Component, ChangeDetectionStrategy } from '@angular/core';
import { TranslateModule } from '@ngx-translate/core';
import { EmptyStateComponent } from '@shared/components/empty-state/empty-state.component';

@Component({
  selector: 'app-veterinary-home',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [TranslateModule, EmptyStateComponent],
  template: `
    <div class="page-container">
      <h1>{{ 'veterinary.title' | translate }}</h1>
      <app-empty-state icon="medical_services" title="veterinary.no_records"></app-empty-state>
    </div>
  `,
  styles: [`.page-container { padding: 24px; max-width: 1200px; margin: 0 auto; }`],
})
export class VeterinaryHomeComponent {}
