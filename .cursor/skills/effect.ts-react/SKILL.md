---
name: effect.ts-react
description: >-
  Expert guidance for integrating Effect.ts with React applications. Covers application layer setup,
  reactive stores (Effect-React bridge), hooks, form handling, authentication patterns, subscriptions,
  and best practices for Effect in React components.
category: framework
displayName: Effect.ts + React Integration
color: cyan
---

# Effect.ts + React Integration Skill

## Purpose

This skill teaches how to integrate Effect.ts with React applications using production-ready patterns. It covers the application layer setup, reactive stores (the Effect-React bridge), custom hooks, form handling with Effect Schema, authentication flows, real-time subscriptions, and best practices for using Effect.ts in React components.

**Use this skill when**: Building React applications with Effect.ts, creating Effect-based hooks, implementing authentication, handling forms with Effect Schema, or setting up real-time subscriptions.

---

## Architecture Overview

### The Effect-React Bridge

Effect.ts and React are fundamentally different paradigms:
- **Effect.ts**: Lazy, composable descriptions of computations (functional effects)
- **React**: Eager, component-based UI rendering

The bridge between them consists of three key pieces:

1. **Application Layer**: Singleton Effect Layer that provides all services
2. **Reactive Stores**: Effect stores that notify React components of state changes
3. **Hooks**: React hooks that run Effects and manage lifecycles

---

## 1. Application Layer Setup

### Pattern: Singleton Application Layer

Create a single application layer that provides all services. This ensures consistent service instances across your app.

```typescript
// lib/appLayer.ts
import { FetchHttpClient, type HttpClient } from '@effect/platform'
import { Effect, Layer, Logger, LogLevel } from 'effect'

// Type union of all services provided by the layer
export type AppServices =
  | HttpClient.HttpClient
  | AuthenticationService
  | EntityApiService
  | RpcApiService
  | ReactiveStore<AuthenticationState>
  // ... other services

const LoggerLayer = Logger.minimumLogLevel(LogLevel.Trace)
const HttpClientLayer = FetchHttpClient.layer

/**
 * Builds the full application layer.
 * Composes all service layers with their dependencies.
 */
export function buildApplicationLayer() {
  const authStoreLayer = getAuthenticationStateStoreLayer()
  
  const AuthLayer = Layer.mergeAll(
    authStoreLayer,
    HttpClientLayer,
    TokenStorageLive,
    AuthenticationLive.pipe(
      Layer.provide(authStoreLayer),
      Layer.provide(HttpClientLayer),
      Layer.provide(TokenStorageLive),
    ),
  )

  const BaseLayer = Layer.mergeAll(
    AuthLayer,
    SubscriptionStreamLive(getBaseUrl()).pipe(
      Layer.provide(authStoreLayer),
      Layer.provide(HttpClientLayer),
    ),
  )

  const ServicesLayer = Layer.mergeAll(
    EntityApiLive,
    RpcApiLive,
    AuditLogLive,
    // ... other service layers
  ).pipe(Layer.provide(BaseLayer))

  return Layer.mergeAll(BaseLayer, ServicesLayer, LoggerLayer)
}

// Cached singleton
let applicationLayer: Layer.Layer<AppServices, never, never> | undefined

/**
 * Returns the singleton application layer.
 * Built once on first call, then cached.
 */
export function getApplicationLayer() {
  if (applicationLayer === undefined) {
    applicationLayer = buildApplicationLayer()
  }
  return applicationLayer
}

/**
 * Hook that returns stable run/runFork functions with the app layer provided.
 * Use these when calling useReactiveStore or running Effects in components.
 */
export function useRunWithAppLayer(): { 
  run: RunEffect
  runFork: RunFork 
} {
  return useMemo(() => {
    const getLayer = () => getApplicationLayer()
    return {
      run: <A, E, R>(effect: Effect.Effect<A, E, R>): Promise<A> =>
        Effect.runPromise(
          effect.pipe(Effect.provide(getLayer())) as Effect.Effect<A, E, never>
        ),
      runFork: <A, E, R>(effect: Effect.Effect<A, E, R>) =>
        Effect.runFork(
          effect.pipe(Effect.provide(getLayer())) as Effect.Effect<A, E, never>
        ),
    }
  }, [])
}
```

### Pattern: Application Bootstrap

Bootstrap the app by running `restoreSession` before mounting React:

```typescript
// main.tsx
import ReactDOM from 'react-dom/client'
import { Effect, pipe } from 'effect'
import { getApplicationLayer } from '@/lib/appLayer'
import { Authentication } from '@/features/authentication/services/Authentication'

await Effect.runPromise(
  pipe(
    Effect.gen(function* () {
      yield* Effect.logInfo('Starting application')

      const auth = yield* Authentication
      yield* auth.restoreSession()

      const rootElement = document.getElementById('root')
      if (rootElement == null) {
        return yield* Effect.fail(new Error('Failed to find the root element'))
      }

      yield* Effect.sync(() => {
        ReactDOM.createRoot(rootElement).render(
          <BrowserRouter>
            <App />
          </BrowserRouter>
        )
      })
    }),
    Effect.provide(getApplicationLayer()),
    Effect.tapError((error) =>
      Effect.sync(() => {
        console.error('Application failed to start:', error)
      })
    ),
  )
)
```

**Why**: Running `restoreSession` before React mounts ensures authentication state is populated before components render, avoiding flicker and unnecessary redirects.

---

## 2. Reactive Stores (Effect-React Bridge)

### Pattern: defineStore for Shared State

Use `defineStore` to create Effect stores that React components can subscribe to:

```typescript
// lib/ReactiveStore.ts
import { Context, Effect, Layer, Stream, Chunk } from 'effect'

/**
 * Reactive store interface: current value, updates, and a stream of changes.
 */
export interface ReactiveStore<A> {
  readonly get: () => Effect.Effect<A, never, never>
  readonly update: (f: (a: A) => A) => Effect.Effect<void, never, never>
  readonly changes: Stream.Stream<A, never, never>
}

/**
 * Defines a reactive store with in-memory state and a Layer.sync.
 * Same instance shared by Effect code and React.
 */
export function defineStore<A>(
  name: string,
  initial: A,
): {
  tag: Context.Tag<ReactiveStore<A>, ReactiveStore<A>>
  layer: Layer.Layer<ReactiveStore<A>, never, never>
} {
  const tag = Context.GenericTag<ReactiveStore<A>>(name)
  
  let current: A = initial
  const changeListeners = new Set<(a: A) => void>()

  const notify = (a: A) => {
    current = a
    // Notify React external store if registered
    if (registry.setter) {
      registry.setter(a)
    }
    changeListeners.forEach((l) => l(a))
  }

  const changes = Stream.async<A, never, never>((emit) => {
    emit(Effect.succeed(Chunk.of(current)))
    const listener = (a: A) => {
      emit(Effect.succeed(Chunk.of(a)))
    }
    changeListeners.add(listener)
    return Effect.sync(() => {
      changeListeners.delete(listener)
    })
  })

  const store: ReactiveStore<A> = {
    get: () => Effect.succeed(current),
    update: (f: (a: A) => A) =>
      Effect.sync(() => {
        notify(f(current))
      }),
    changes,
  }

  const layer = Layer.sync(tag, () => store)
  return { tag, layer }
}
```

### Pattern: Authentication State Store

Example of defining a reactive store for authentication state:

```typescript
// features/authentication/stores/AuthenticationStateReactiveStore.ts
import { Option } from 'effect'
import { defineStore } from '@/lib/ReactiveStore'
import type { AuthenticationUser } from '../domain/AuthenticationUser'

export interface AuthenticationState {
  readonly token: Option.Option<string>
  readonly user: Option.Option<AuthenticationUser>
  readonly needsScopeSelect: Option.Option<boolean>
  readonly permissions: readonly string[]
}

export const initialAuthenticationState: AuthenticationState = {
  token: Option.none(),
  user: Option.none(),
  needsScopeSelect: Option.none(),
  permissions: [],
}

export const { tag: AuthStoreTag, layer: AuthStoreLayer } = defineStore(
  'AuthenticationStateReactiveStore',
  initialAuthenticationState,
)

export function getAuthenticationStateStoreLayer() {
  return AuthStoreLayer
}
```

### Pattern: Reactive Store Hooks

Create bound hooks for your stores to simplify usage in components:

```typescript
// features/authentication/hooks/useAuthenticationStateReactiveStore.ts
import { useRunWithAppLayer } from '@/lib/appLayer'
import { useReactiveStore, useReactiveStoreWithInit } from '@/lib/ReactiveStore'
import { AuthStoreTag, initialAuthenticationState } from '../stores/AuthenticationStateReactiveStore'

/** Returns current auth state only */
export function useAuthStore(): AuthenticationState {
  const { run, runFork } = useRunWithAppLayer()
  return useReactiveStore(
    AuthStoreTag,
    initialAuthenticationState,
    run,
    runFork,
  )
}

/** Returns auth state and initialized flag (for protected routes) */
export function useAuthStoreWithInit(): {
  authentication: AuthenticationState
  initialized: boolean
} {
  const { run, runFork } = useRunWithAppLayer()
  const snapshot = useReactiveStoreWithInit(
    AuthStoreTag,
    initialAuthenticationState,
    run,
    runFork,
  )
  return {
    authentication: snapshot.value,
    initialized: snapshot.initialized,
  }
}
```

**Why**: Bound hooks encapsulate the `useRunWithAppLayer` setup and make it trivial to use stores in components.

---

## 3. Effect-Based Hooks

### Pattern: usePermission Hook

Example of a hook that derives state from a reactive store:

```typescript
// hooks/usePermission.ts
import { useAuthStore } from '@/features/authentication/stores'
import { hasPermission } from '@/lib/permissions'

/**
 * Returns whether the current user has the required permission(s).
 * Uses state.permissions from auth store.
 */
export function usePermission(required: string | readonly string[]): boolean {
  const authentication = useAuthStore()
  return hasPermission(authentication.permissions, required)
}
```

### Pattern: Running Effects in Components

When you need to run an Effect from a component (event handler, submit, etc.):

```typescript
// In a component
import { Effect } from 'effect'
import { useRunWithAppLayer } from '@/lib/appLayer'
import { Authentication } from '@/features/authentication/services/Authentication'

export function MyComponent() {
  const { run } = useRunWithAppLayer()
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const handleAction = useCallback(() => {
    const effect = Effect.gen(function* () {
      const auth = yield* Authentication
      const user = yield* auth.getCurrentUser()
      // ... do something with user
      return user
    })

    setLoading(true)
    setError(null)

    run(effect)
      .then((result) => {
        // Handle success
        setLoading(false)
      })
      .catch((err) => {
        setError(err.message)
        setLoading(false)
      })
  }, [run])

  return (
    <button onClick={handleAction} disabled={loading}>
      {loading ? 'Loading...' : 'Click me'}
    </button>
  )
}
```

**Key Points**:
- Use `useRunWithAppLayer()` to get a `run` function that provides the app layer
- Build your Effect program using `Effect.gen` and service tags
- Call `run(effect)` to get a Promise
- Use React state to track loading/error states

---

## 4. Form Handling with Effect Schema

### Pattern: useForm Hook

Create a generalized Effect-based form hook:

```typescript
// hooks/useForm.ts
import { Schema, Effect, Either, ParseResult } from 'effect'
import { useState, useCallback } from 'react'

export interface FieldError {
  readonly field: string
  readonly message: string
}

export interface FormState<TValues extends Record<string, unknown>> {
  readonly values: TValues
  readonly errors: readonly FieldError[]
  readonly isValid: boolean
  readonly touched: Partial<Record<keyof TValues, boolean>>
}

export interface UseFormConfig<TSchema extends Schema.Schema<any, any, never>> {
  readonly schema: TSchema
  readonly initialValues: Schema.Schema.Type<TSchema>
}

/**
 * Effect.ts-based form hook.
 * Validates using Effect.ts Schema and provides Effect-based handlers.
 * Form values type is inferred from schema.
 */
export function useForm<TSchema extends Schema.Schema<any, any, never>>(
  config: UseFormConfig<TSchema>,
) {
  type TValues = Schema.Schema.Type<TSchema>
  
  const [formState, setFormState] = useState<FormState<TValues>>({
    values: config.initialValues,
    errors: [],
    isValid: false,
    touched: {},
  })

  /**
   * Validates form values using Effect.ts Schema.
   * Returns Effect that succeeds with validated data or fails with errors.
   */
  const validateForm = useCallback(
    (values: TValues) => {
      const result = Schema.decodeUnknownEither(config.schema, {
        errors: 'all',
      })(values)

      if (Either.isLeft(result)) {
        const issues = ParseResult.ArrayFormatter.formatErrorSync(result.left)
        const errors = issues.map((issue) => ({
          field: issue.path.length > 0 ? String(issue.path[0]) : 'root',
          message: issue.message,
        }))
        return Effect.fail(errors)
      }

      return Effect.succeed(result.right)
    },
    [config.schema],
  )

  const setFieldValue = useCallback(
    <K extends keyof TValues>(field: K, value: TValues[K]) => {
      setFormState((prev) => ({
        ...prev,
        values: { ...prev.values, [field]: value },
      }))
    },
    [],
  )

  const setFieldTouched = useCallback(<K extends keyof TValues>(field: K) => {
    setFormState((prev) => ({
      ...prev,
      touched: { ...prev.touched, [field]: true },
    }))
  }, [])

  const getFieldError = useCallback(
    (field: string): string | undefined => {
      const error = formState.errors.find((e) => e.field === field)
      return error?.message
    },
    [formState.errors],
  )

  const setValidationErrors = useCallback((errors: readonly FieldError[]) => {
    setFormState((prev) => {
      const touchedFields = errors.reduce(
        (acc, error) => {
          if (error.field !== 'root') {
            acc[error.field as keyof TValues] = true
          }
          return acc
        },
        { ...prev.touched } as Partial<Record<keyof TValues, boolean>>,
      )

      return {
        ...prev,
        errors,
        isValid: errors.length === 0,
        touched: touchedFields,
      }
    })
  }, [])

  return {
    formState,
    setFieldValue,
    setFieldTouched,
    getFieldError,
    validateForm,
    setValidationErrors,
  }
}
```

### Pattern: Login Form with Effect

Example of using the form hook in a login component:

```typescript
// features/authentication/pages/LoginPage.tsx
import { Effect, Either } from 'effect'
import { useNavigate } from 'react-router-dom'
import { useCallback, useState } from 'react'
import { useForm } from '@/hooks/useForm'
import { LoginFormSchema } from '@/features/authentication/schemas/LoginFormSchema'
import { Authentication } from '@/features/authentication/services/Authentication'
import { getApplicationLayer } from '@/lib/appLayer'
import { navigateTo } from '@/lib/navigate'

export default function LoginPage() {
  const navigate = useNavigate()
  const [submitting, setSubmitting] = useState(false)
  const [errorMessage, setErrorMessage] = useState<string | null>(null)

  const {
    formState,
    setFieldValue,
    setFieldTouched,
    getFieldError,
    validateForm,
    setValidationErrors,
  } = useForm({
    schema: LoginFormSchema,
    initialValues: {
      email: '',
      password: '',
    },
  })

  const handleSubmit = useCallback(
    (e: React.FormEvent<HTMLFormElement>) => {
      e.preventDefault()

      const currentValues = formState.values
      
      // Validate before submit
      const validationResult = Effect.runSync(
        validateForm(currentValues).pipe(
          Effect.either,
          Effect.map((either) =>
            Either.match(either, {
              onLeft: (errors) => ({ errors, isValid: false }),
              onRight: () => ({ errors: [], isValid: true }),
            }),
          ),
        ),
      )

      setValidationErrors(validationResult.errors)
      if (!validationResult.isValid) return

      const submitEffect = Effect.gen(function* () {
        const validated = yield* validateForm(currentValues)
        const auth = yield* Authentication
        yield* auth.login(validated.email, validated.password)
        yield* navigateTo(navigate, '/dashboard', { replace: true })
        return { type: 'success' as const }
      }).pipe(
        Effect.catchAll((error) =>
          Effect.gen(function* () {
            if (error instanceof AuthenticationError) {
              return yield* Effect.succeed({
                type: 'authentication' as const,
                message: error.message,
              })
            }
            return yield* Effect.succeed({
              type: 'unknown' as const,
              message: 'Login failed',
            })
          }),
        ),
        Effect.provide(getApplicationLayer()),
      )

      setSubmitting(true)
      setErrorMessage(null)

      Effect.runPromise(submitEffect).then((result) => {
        setSubmitting(false)
        if (result.type === 'authentication' || result.type === 'unknown') {
          setErrorMessage(result.message)
        }
      })
    },
    [navigate, formState.values, validateForm, setValidationErrors],
  )

  return (
    <form onSubmit={handleSubmit}>
      <TextField
        name="email"
        value={formState.values.email}
        onChange={(e) => setFieldValue('email', e.target.value)}
        onBlur={() => setFieldTouched('email')}
        error={Boolean(getFieldError('email'))}
        helperText={getFieldError('email')}
        disabled={submitting}
      />
      <TextField
        name="password"
        type="password"
        value={formState.values.password}
        onChange={(e) => setFieldValue('password', e.target.value)}
        onBlur={() => setFieldTouched('password')}
        error={Boolean(getFieldError('password'))}
        helperText={getFieldError('password')}
        disabled={submitting}
      />
      {errorMessage && <Alert severity="error">{errorMessage}</Alert>}
      <Button type="submit" disabled={submitting}>
        {submitting ? 'Logging in...' : 'Log in'}
      </Button>
    </form>
  )
}
```

**Key Points**:
- Schema validation happens in Effect (not React state)
- Validation errors are extracted and stored in React state for UI display
- Submit handler builds an Effect pipeline that validates → authenticates → navigates
- Loading/error states are managed with React hooks

---

## 5. Authentication Patterns

### Pattern: Protected Routes

Use the initialized flag to avoid redirecting before state is loaded:

```typescript
// components/ProtectedRoute.tsx
import React from 'react'
import { Navigate } from 'react-router-dom'
import { Option } from 'effect'
import { useAuthStoreWithInit } from '@/features/authentication/stores'

export function ProtectedRoute({ children }: { children: React.ReactNode }) {
  const { authentication, initialized } = useAuthStoreWithInit()
  const isUserAuthenticated = Option.isSome(authentication.user)

  // Don't redirect until store is initialized (after restoreSession)
  if (!initialized) {
    return null
  }

  if (!isUserAuthenticated) {
    return <Navigate to="/authentication/login" replace />
  }

  return <>{children}</>
}
```

### Pattern: Permission Guards

Components that conditionally render based on permissions:

```typescript
// components/ShowWithPermissions.tsx
import { usePermission } from '@/hooks/usePermission'

export function ShowWithPermissions({
  required,
  children,
}: {
  required: string | readonly string[]
  children: React.ReactNode
}) {
  const hasPermission = usePermission(required)
  return hasPermission ? <>{children}</> : null
}

// Usage
<ShowWithPermissions required="users.write">
  <Button>Create User</Button>
</ShowWithPermissions>
```

---

## 6. Real-Time Subscriptions

### Pattern: Subscription Stream Runner

Mount a component that runs the subscription stream Effect when authenticated:

```typescript
// components/SubscriptionStreamRunner.tsx
import { useEffect } from 'react'
import { Effect, Stream, Option } from 'effect'
import { useAuthStore } from '@/features/authentication/stores'
import { getApplicationLayer } from '@/lib/appLayer'
import { trigger } from '@/lib/subscriptionRegistry'
import { SubscriptionStream } from '@/services/SubscriptionStream'
import { SubscriptionStreamStatusStoreTag } from '@/lib/subscriptionStreamStatusStore'

export function SubscriptionStreamRunner() {
  const authentication = useAuthStore()
  const hasToken = Option.isSome(authentication.token)

  useEffect(() => {
    if (!hasToken) return

    const program = Effect.gen(function* () {
      const svc = yield* SubscriptionStream
      const statusStore = yield* SubscriptionStreamStatusStoreTag
      const stream = yield* svc.openStream()

      yield* Effect.fork(
        Stream.runForEach(stream, (e) =>
          Effect.gen(function* () {
            if ('type' in e && e.type === 'ready') {
              yield* statusStore.update(() => ({ connected: true }))
            }
            if ('subscription_id' in e) {
              trigger(e.subscription_id)
            }
          }),
        ).pipe(
          Effect.ensuring(statusStore.update(() => ({ connected: false }))),
        ),
      )
    })

    Effect.runFork(program.pipe(Effect.provide(getApplicationLayer())))
  }, [hasToken])

  return null
}
```

### Pattern: Entity Subscription Hook

Hook that subscribes to entity updates and triggers refetch:

```typescript
// hooks/useEntitySubscription.ts
import { useEffect, useRef } from 'react'
import { Effect, Option } from 'effect'
import { useAuthStore } from '@/features/authentication/stores'
import { useRunWithAppLayer } from '@/lib/appLayer'
import { register, unregister } from '@/lib/subscriptionRegistry'
import { RpcApi } from '@/services/RpcApi'

export function useEntitySubscription(
  entityId: string,
  params: ListQueryParams | undefined,
  onRefetch: () => void,
): void {
  const authentication = useAuthStore()
  const hasToken = Option.isSome(authentication.token)
  const subscriptionIdRef = useRef<string | null>(null)
  const onRefetchRef = useRef(onRefetch)
  onRefetchRef.current = onRefetch
  const { run } = useRunWithAppLayer()

  useEffect(() => {
    if (!hasToken) return

    const subscribeEffect = Effect.gen(function* () {
      const rpc = yield* RpcApi
      return yield* rpc.subscribe(entityId, params)
    })

    run(subscribeEffect)
      .then((result) => {
        subscriptionIdRef.current = result.subscription_id
        register(result.subscription_id, {
          entityId,
          params: params ?? undefined,
          onInvalidate: () => onRefetchRef.current(),
        })
      })
      .catch(() => {})

    return () => {
      const id = subscriptionIdRef.current
      if (id) {
        unregister(id)
        subscriptionIdRef.current = null
      }
    }
  }, [entityId, hasToken, run, JSON.stringify(params ?? {})])
}
```

---

## 7. Best Practices

### ✅ DO: Run Effects at Boundaries

Run Effects in:
- Event handlers (onClick, onSubmit)
- useEffect hooks
- Application bootstrap (main.tsx)

```typescript
// ✅ GOOD
const handleClick = () => {
  const effect = Effect.gen(function* () {
    const auth = yield* Authentication
    yield* auth.logout()
  })
  
  run(effect).then(() => {
    // Handle success
  })
}
```

### ✅ DO: Use Reactive Stores for Shared State

Use reactive stores for state that:
- Needs to be accessed by multiple components
- Is managed by Effect services
- Should persist across navigation

```typescript
// ✅ GOOD - Authentication state in reactive store
const { authentication, initialized } = useAuthStoreWithInit()
```

### ✅ DO: Keep Components Simple

Components should:
- Subscribe to reactive stores
- Call Effects via event handlers
- Manage UI-only state with useState

```typescript
// ✅ GOOD - Component only handles UI concerns
export function MyComponent() {
  const authentication = useAuthStore()
  const { run } = useRunWithAppLayer()
  const [loading, setLoading] = useState(false)
  
  // Component logic here
}
```

### ❌ DON'T: Run Effects During Render

Never run Effects during component render:

```typescript
// ❌ BAD - Running Effect during render
export function MyComponent() {
  const { run } = useRunWithAppLayer()
  
  // This runs on every render!
  run(someEffect)
  
  return <div>...</div>
}

// ✅ GOOD - Run in useEffect
export function MyComponent() {
  const { run } = useRunWithAppLayer()
  
  useEffect(() => {
    run(someEffect)
  }, [run])
  
  return <div>...</div>
}
```

### ❌ DON'T: Mix React State with Effect State

Don't duplicate state in both React and Effect:

```typescript
// ❌ BAD - Duplicating user in React state
const [user, setUser] = useState(null)
const authentication = useAuthStore() // Also has user

// ✅ GOOD - Single source of truth
const authentication = useAuthStore()
const user = Option.getOrNull(authentication.user)
```

### ❌ DON'T: Create Multiple Application Layers

Always use the singleton layer from `getApplicationLayer()`:

```typescript
// ❌ BAD - Creating new layers
const myLayer = buildApplicationLayer()
run(effect.pipe(Effect.provide(myLayer)))

// ✅ GOOD - Use singleton
const layer = getApplicationLayer()
run(effect.pipe(Effect.provide(layer)))
```

---

## 8. Testing Patterns

### Pattern: Test Layer Override

Override the application layer for tests:

```typescript
// In test setup
import { setApplicationLayerOverrideForTesting } from '@/lib/appLayer'
import { Layer } from 'effect'
import { AuthenticationMock } from '@/features/authentication/services/AuthenticationMock'

beforeEach(() => {
  const testLayer = Layer.mergeAll(
    AuthenticationMock,
    // ... other mock layers
  )
  setApplicationLayerOverrideForTesting(testLayer)
})

afterEach(() => {
  clearApplicationLayerOverrideForTesting()
})
```

### Pattern: Testing Components with Reactive Stores

```typescript
import { render, screen } from '@testing-library/react'
import { BrowserRouter } from 'react-router-dom'

// Component tests automatically use the test layer override
it('should show login page', () => {
  render(
    <BrowserRouter>
      <LoginPage />
    </BrowserRouter>
  )
  expect(screen.getByText('Log in')).toBeInTheDocument()
})
```

---

## Quick Reference

| Pattern | Use When |
|---------|----------|
| `defineStore` | Creating shared state between Effect and React |
| `useRunWithAppLayer` | Running Effects from components |
| `useAuthStore` | Accessing authentication state |
| `usePermission` | Checking permissions in components |
| `useForm` | Building forms with Effect Schema validation |
| `ProtectedRoute` | Guarding routes requiring authentication |
| `SubscriptionStreamRunner` | Setting up real-time subscriptions |
| `useEntitySubscription` | Subscribing to entity updates |

---

## Related Skills

- `effect.ts-fundamentals` - Effect as value, pipe/flatMap, FP data types
- `effect.ts-architect` - Layers, Services, dependency injection
- `effect.ts-testing` - Testing Effect.ts code
- `typescript-expert` - TypeScript best practices

---

**Remember**: The key to Effect + React integration is the **reactive store bridge**. Effect services update stores, React components subscribe to stores, and hooks run Effects at boundaries. Keep this separation clean and your app will be maintainable and type-safe.
