import { Component, OnInit, inject } from '@angular/core';
import { Router } from '@angular/router';
import { AuthService } from '../../core/services/auth.service';

@Component({
  selector: 'app-dashboard-redirect',
  standalone: true,
  template: `<p>{{ 'common.loading' }}</p>`,
})
export class DashboardRedirectComponent implements OnInit {
  private readonly auth = inject(AuthService);
  private readonly router = inject(Router);

  ngOnInit(): void {
    const user = this.auth.currentUser();
    if (!user) {
      this.router.navigate(['/auth/login']);
      return;
    }

    switch (user.role) {
      case 'eleveur':
        this.router.navigate(['/dashboard/eleveur']);
        break;
      case 'producteur_aliment':
        this.router.navigate(['/dashboard/producteur']);
        break;
      case 'admin':
        this.router.navigate(['/dashboard/admin']);
        break;
      case 'client':
      default:
        this.router.navigate(['/dashboard/client']);
        break;
    }
  }
}
