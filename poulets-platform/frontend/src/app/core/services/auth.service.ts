import { Injectable, inject, signal, computed } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { Router } from '@angular/router';
import { Observable, tap, catchError, of } from 'rxjs';
import { environment } from '@env/environment';
import { UserSession, Role, LoginRequest, RegisterRequest } from '@app/shared/models/user.model';

export interface MenuItem {
  icon: string;
  labelKey: string;
  route: string;
}

const ELEVEUR_MENU: MenuItem[] = [
  { icon: 'dashboard', labelKey: 'menu.dashboard', route: '/dashboard' },
  { icon: 'storefront', labelKey: 'menu.my_listings', route: '/marketplace' },
  { icon: 'inventory_2', labelKey: 'menu.my_lots', route: '/growth' },
  { icon: 'shopping_cart', labelKey: 'menu.orders', route: '/orders' },
  { icon: 'medical_services', labelKey: 'menu.veterinary', route: '/veterinary' },
  { icon: 'verified', labelKey: 'menu.halal_certification', route: '/halal' },
  { icon: 'description', labelKey: 'menu.contracts', route: '/contracts' },
  { icon: 'chat', labelKey: 'menu.messaging', route: '/messaging' },
  { icon: 'person', labelKey: 'menu.profile', route: '/profile' },
];

const CLIENT_MENU: MenuItem[] = [
  { icon: 'dashboard', labelKey: 'menu.dashboard', route: '/dashboard' },
  { icon: 'storefront', labelKey: 'menu.marketplace', route: '/marketplace' },
  { icon: 'assignment', labelKey: 'menu.publish_need', route: '/marketplace' },
  { icon: 'shopping_cart', labelKey: 'menu.my_orders', route: '/orders' },
  { icon: 'event', labelKey: 'menu.delivery_calendar', route: '/calendar' },
  { icon: 'description', labelKey: 'menu.contracts', route: '/contracts' },
  { icon: 'chat', labelKey: 'menu.messaging', route: '/messaging' },
  { icon: 'person', labelKey: 'menu.profile', route: '/profile' },
];

const PRODUCTEUR_MENU: MenuItem[] = [
  { icon: 'dashboard', labelKey: 'menu.dashboard', route: '/dashboard' },
  { icon: 'request_page', labelKey: 'menu.aggregated_demand', route: '/marketplace' },
  { icon: 'inventory', labelKey: 'menu.my_products', route: '/growth' },
  { icon: 'shopping_cart', labelKey: 'menu.orders', route: '/orders' },
  { icon: 'chat', labelKey: 'menu.messaging', route: '/messaging' },
  { icon: 'person', labelKey: 'menu.profile', route: '/profile' },
];

const ADMIN_MENU: MenuItem[] = [
  { icon: 'dashboard', labelKey: 'menu.global_view', route: '/dashboard' },
  { icon: 'people', labelKey: 'menu.users', route: '/profile' },
  { icon: 'receipt_long', labelKey: 'menu.transactions', route: '/orders' },
  { icon: 'bar_chart', labelKey: 'menu.statistics', route: '/dashboard' },
  { icon: 'map', labelKey: 'menu.map', route: '/map' },
];

@Injectable({ providedIn: 'root' })
export class AuthService {
  private readonly http = inject(HttpClient);
  private readonly router = inject(Router);
  private readonly bffUrl = environment.bffUrl;

  // Reactive state
  private readonly _user = signal<UserSession | null>(null);
  private readonly _loading = signal(false);
  private readonly _initialized = signal(false);

  // Public computed signals
  readonly currentUser = this._user.asReadonly();
  readonly loading = this._loading.asReadonly();
  readonly initialized = this._initialized.asReadonly();

  readonly isLoggedIn = computed(() => this._user() !== null);
  readonly isAuthenticated = computed(() => this._user() !== null);
  readonly userRole = computed(() => this._user()?.role ?? null);
  readonly isEleveur = computed(() => this._user()?.role === 'eleveur');
  readonly isClient = computed(() => this._user()?.role === 'client');
  readonly isProducteur = computed(() => this._user()?.role === 'producteur_aliment');
  readonly isAdmin = computed(() => this._user()?.role === 'admin');

  readonly menuItems = computed<MenuItem[]>(() => {
    const role = this._user()?.role;
    switch (role) {
      case 'eleveur': return ELEVEUR_MENU;
      case 'client': return CLIENT_MENU;
      case 'producteur_aliment': return PRODUCTEUR_MENU;
      case 'admin': return ADMIN_MENU;
      default: return [];
    }
  });

  readonly spaceLabel = computed<string>(() => {
    const role = this._user()?.role;
    switch (role) {
      case 'eleveur': return 'menu.eleveur_space';
      case 'client': return 'menu.client_space';
      case 'producteur_aliment': return 'menu.producer_space';
      case 'admin': return 'menu.admin_space';
      default: return '';
    }
  });

  /**
   * Check if an active session exists (cookie-based via BFF).
   * Called on application bootstrap.
   */
  checkSession(): void {
    this._loading.set(true);
    this.http
      .get<UserSession>(`${this.bffUrl}/api/auth/session`, {
        withCredentials: true,
      })
      .pipe(
        tap((user) => {
          this._user.set(user);
          this._loading.set(false);
          this._initialized.set(true);
        }),
        catchError(() => {
          this._user.set(null);
          this._loading.set(false);
          this._initialized.set(true);
          return of(null);
        }),
      )
      .subscribe();
  }

  /**
   * Login via BFF which proxies to Kratos.
   */
  login(credentials: LoginRequest): Observable<UserSession> {
    this._loading.set(true);
    return this.http
      .post<UserSession>(`${this.bffUrl}/api/auth/login`, credentials, {
        withCredentials: true,
      })
      .pipe(
        tap((user) => {
          this._user.set(user);
          this._loading.set(false);
        }),
        catchError((err) => {
          this._loading.set(false);
          throw err;
        }),
      );
  }

  /**
   * Register a new account via BFF -> Kratos.
   */
  register(data: RegisterRequest): Observable<UserSession> {
    this._loading.set(true);
    return this.http
      .post<UserSession>(`${this.bffUrl}/api/auth/register`, data, {
        withCredentials: true,
      })
      .pipe(
        tap((user) => {
          this._user.set(user);
          this._loading.set(false);
        }),
        catchError((err) => {
          this._loading.set(false);
          throw err;
        }),
      );
  }

  /**
   * Request a password reset email.
   */
  forgotPassword(email: string): Observable<void> {
    return this.http.post<void>(
      `${this.bffUrl}/api/auth/forgot-password`,
      { email },
      { withCredentials: true },
    );
  }

  /**
   * Logout: destroy session cookie via BFF.
   */
  logout(): Observable<void> {
    return this.http
      .post<void>(`${this.bffUrl}/api/auth/logout`, {}, {
        withCredentials: true,
      })
      .pipe(
        tap(() => {
          this._user.set(null);
        }),
        catchError(() => {
          this._user.set(null);
          return of(void 0);
        }),
      );
  }

  /**
   * Navigate user to their default dashboard route based on role.
   */
  navigateByRole(): void {
    const role = this._user()?.role;
    switch (role) {
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

  /**
   * Check whether the user has a specific role.
   */
  hasRole(role: Role): boolean {
    return this._user()?.role === role;
  }

  /**
   * Check whether the user has any of the specified roles.
   */
  hasAnyRole(...roles: Role[]): boolean {
    const currentRole = this._user()?.role;
    return currentRole != null && roles.includes(currentRole);
  }
}
