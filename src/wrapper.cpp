#include "wrapper.hpp"
#include <generated_wrapper.cpp>

int64_t Delegate::Call(wxEvent *event) {
  if (m_function_pointer) {
    return (*m_function_pointer)(m_data_pointer, event);
  } else {
    return 0;
  }
}

void Delegate::Thunk(wxEvent& event) {
    (*m_function_pointer)(m_data_pointer, &event);
}

Delegate *globalAppInitDelegate;

void setGlobalAppInitDelegate(Delegate *delegate) {
  globalAppInitDelegate = delegate;
}

class App: public wxApp {
public:
  /**
   * We need to overwrite this, which is the main reason why this class exists.
   */
  virtual bool OnInit();
};

void BindDelegate(
    wxEvtHandler *eventSource,
    wxEventType eventType,
    Delegate *delegate,
    int id,
    int lastId) {
  eventSource->Connect(
      id, lastId, eventType, (wxObjectEventFunction)
      &Delegate::Thunk, nullptr, delegate);
}

void UnBindDelegate(
    wxEvtHandler *eventSource,
    wxEventType eventType,
    Delegate *delegate) {
  eventSource->Disconnect(
      eventType, (wxObjectEventFunction) &Delegate::Thunk, nullptr, delegate);
}

wxIMPLEMENT_APP_NO_MAIN(App);

bool App::OnInit()
{
    return !!globalAppInitDelegate->Call(nullptr);
}


/***
 * Workarounds for generated wrapper not being complete.
 * This is *mostly* due to the wxWidgets documentation being wrong.
 */
wxMenuBar* construct_wxMenuBar() {
  return new wxMenuBar;
}
