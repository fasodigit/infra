import { Injectable, inject, signal, computed } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { Observable, tap, catchError, of, map } from 'rxjs';
import { environment } from '@env/environment';

export interface UserSession {
  id: string;
  email: string;
  name: string;
  role: 'client' | 'eleveur' | 'admin';
  verified: boolean;
}

export interface LoginRequest {
  email: string;
  password: string;
}

export interface RegisterRequest {
  email: string;
  password: string;
  name: string;
  role: 'client' | 'eleveur';
  phone?: string;
}

@Injectable({ providedIn: 'root' })
export class AuthService {
  private readonly http = inject(HttpClient);
  private readonly bffUrl = environment.bffUrl;

  // Reactive state using Angular signals
  private readonly _user = signal<UserSession | null>(null);
  private readonly _loading = signal(false);
  private readonly _initialized = signal(false);

  // Public computed signals
  readonly currentUser = this._user.asReadonly();
  readonly loading = this._loading.asReadonly();
  readonly initialized = this._initialized.asReadonly();

  readonly isAuthenticated = computed(() => this._user() !== null);
  readonly isEleveur = computed(() => this._user()?.role === 'eleveur');
  readonly isAdmin = computed(() => this._user()?.role === 'admin');

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
   * BFF sets httpOnly session cookie on success.
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
}
