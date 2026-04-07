import { Component, OnInit, inject } from '@angular/core';
import { RouterOutlet } from '@angular/router';
import { TranslateService } from '@ngx-translate/core';
import { AuthService } from './core/services/auth.service';

@Component({
  selector: 'app-root',
  standalone: true,
  imports: [RouterOutlet],
  template: `<router-outlet />`,
})
export class AppComponent implements OnInit {
  private readonly auth = inject(AuthService);
  private readonly translate = inject(TranslateService);

  ngOnInit(): void {
    // Initialize i18n: set French as default and active language
    this.translate.setDefaultLang('fr');
    this.translate.use('fr');

    this.auth.checkSession();
  }
}
